# rlnk 개발 작업계획

이 문서는 `doc/PLANNING.md`의 요구사항을 실제 Rust 애플리케이션으로 구현하기 위한 작업 순서와 품질 기준을 정리한다. 목표는 MongoDB를 사용하는 초소형 고성능 URL shortener API를 안정적으로 만들고, 이후 기능 추가가 쉬운 구조를 갖추는 것이다.

## 1. 프로젝트 뼈대 생성

- `cargo init --bin`으로 Rust 바이너리 프로젝트를 생성한다.
- 비즈니스 로직 테스트가 가능하도록 실행 진입점은 `src/main.rs`, 실제 구현은 `src/lib.rs`와 하위 모듈에 둔다.
- 기본 모듈 구조를 다음처럼 잡는다.
  - `config`: 환경변수 로딩과 검증
  - `http`: 라우터, 핸들러, 요청/응답 DTO
  - `auth`: `Authorization` 헤더 검증
  - `store`: MongoDB 접근 추상화
  - `model`: 도메인 타입과 MongoDB 문서 타입
  - `error`: 애플리케이션 오류 타입과 HTTP 오류 변환

## 2. 기술 스택 확정

- HTTP 서버는 `axum`과 `tokio`를 사용한다.
- MongoDB 접근은 공식 `mongodb` crate를 사용한다.
- 직렬화와 역직렬화는 `serde`를 사용한다.
- 시간 처리는 MongoDB BSON `DateTime`과 `humantime`을 사용한다. TTL 파싱은 duration 문자열을 `std::time::Duration`으로 해석하고, 응답 시각은 RFC3339 문자열로 반환한다.
- 환경변수 로딩은 애플리케이션에서 직접 `std::env`를 읽는다. `.env` 로딩은 애플리케이션에서 처리하지 않고 Docker Compose의 `env_file`로 커버한다. `.env.sample`을 제공한다.
- 오류 모델링은 `thiserror` 기반의 명시적 오류 enum을 사용한다.
- 요청 추적과 운영 로그는 `tracing`과 `tracing-subscriber`를 사용한다.

## 3. 설정과 실행 환경 구현

- 필수 환경변수를 정의한다.
  - `MONGO_URI`: MongoDB 연결 문자열
  - `APP_KEY`: `POST /gen`, `DELETE /{hash}` 보호용 키
  - `APP_HOSTNAME`: 생성된 short URL의 외부 호스트명
- 선택 환경변수를 정의한다.
  - `APP_BIND_ADDR`: 기본값 `0.0.0.0:8080`
  - `MONGO_DATABASE`: 기본값 `rlnk`
  - `MONGO_COLLECTION`: 기본값 `links`
  - `HASH_LENGTH`: 기본값은 충돌 가능성과 URL 길이를 고려해 정한다.
  - `ACCESS_CACHE_SIZE`: 최근 접근한 링크 정보를 메모리에 보관할 최대 항목 수, 기본값 `1024`, `0`이면 비활성화
- 설정 로딩 실패는 프로세스 시작 단계에서 명확한 오류로 종료한다.

## 4. 데이터 모델 설계

- 링크 문서는 다음 필드를 가진다.
  - `hash`: short URL 식별자, unique index
  - `original_url`: 원본 URL
  - `created_at`: 생성 일자
  - `expires_at`: TTL이 있을 때만 저장되는 만료 일자
  - `access_count`: 접근 횟수
  - `last_accessed_at`: 마지막 접근 일자
- MongoDB index를 준비한다.
  - `hash` unique index
  - `expires_at` TTL index. 만료 없음 링크와 공존할 수 있도록 nullable 필드로 설계한다.
- Rust 타입은 API DTO와 DB document 타입을 분리해서 MongoDB 세부사항이 HTTP 계층으로 새지 않게 한다.

## 5. 해시 생성 정책 구현

- 생성 결과는 `APP_HOSTNAME`과 `hash`를 결합한 URL로 반환한다.
- 해시는 URL-safe alphabet을 사용한다.
- 충돌 발생 시 제한된 횟수만 재시도하고, 모두 실패하면 서버 오류로 반환한다.
- 해시 생성 함수는 외부 상태 없이 테스트 가능하게 분리한다.
- 원본 URL 검증은 최소한 scheme이 있는 HTTP/HTTPS URL만 허용하는 방향으로 시작한다.

## 6. API 구현

- `POST /gen`
  - `Authorization` 헤더를 검증한다.
  - 요청 body에서 원본 링크와 선택 TTL을 받는다.
  - URL과 TTL을 검증한다.
  - MongoDB에 링크 문서를 저장한다.
  - 생성된 short URL과 hash를 반환한다.
- `DELETE /{hash}`
  - `Authorization` 헤더를 검증한다.
  - hash에 해당하는 문서를 삭제한다.
  - 존재하지 않는 hash는 `404 Not Found`로 반환한다.
- `GET /{hash}`
  - hash에 해당하는 문서를 조회한다.
  - 만료된 링크는 `404 Not Found`로 반환한다.
  - 접근 횟수와 마지막 접근 일자를 원자적으로 갱신한다.
  - 최근 접근 캐시에 있는 hash는 원본 URL과 만료 시각을 메모리에서 읽고, DB에는 접근 통계 원자 업데이트만 수행한다.
  - 원본 URL로 리다이렉트한다.
- `GET /stat`
  - `Authorization` 헤더를 검증한다.
  - 생성된 링크들의 hash, short URL, 원본 링크, 접근 횟수, 생성 시각, 만료 시각, 마지막 접근 일자를 반환한다.
  - 초기 버전은 전체 목록을 반환하되, 데이터가 늘어나는 상황을 고려해 pagination 추가 지점을 남긴다.

## 7. 인증 정책

- `POST /gen`, `DELETE /{hash}`, `GET /stat`는 `Authorization: Bearer <APP_KEY>` 헤더가 필요하다.
- 초기 구현은 단일 shared secret 방식으로 단순하게 유지한다.
- 인증 실패는 `401 Unauthorized`로 반환하고, 내부 키 값은 로그나 응답에 노출하지 않는다.
- 비교 방식은 불필요한 문자열 복제를 피하고, 필요하면 constant-time 비교 crate 도입을 검토한다.

## 8. 오류 처리와 응답 형식

- 내부 오류는 `thiserror` enum으로 모델링한다.
- 핸들러는 `Result<T, AppError>`를 반환하고 `IntoResponse` 구현으로 HTTP 상태와 JSON 오류 응답을 통일한다.
- 생산 코드에서 `unwrap()`과 `expect()`를 사용하지 않는다.
- 외부 시스템 오류 MongoDB, 환경변수, URL parse 오류는 계층별 오류로 변환한다.
- 클라이언트 입력 오류와 서버 내부 오류가 섞이지 않도록 상태 코드를 명확히 분리한다.

## 9. 테스트 계획

- 단위 테스트
  - 환경변수 파싱과 기본값 적용
  - Authorization 헤더 검증
  - URL과 TTL 검증
  - 해시 생성 길이와 alphabet 보장
  - 만료 판단 로직
- 통합 테스트
  - `POST /gen` 성공과 인증 실패
  - `GET /{hash}` 리다이렉트와 접근 카운트 증가
  - `DELETE /{hash}` 삭제 후 조회 실패
  - `GET /stat` 응답 구조
- HTTP 통합 테스트는 `MemoryLinkStore` 기반으로 수행하고, 실제 MongoDB 연동 검증은 별도 smoke test로 남긴다.
- 테스트 이름은 동작과 조건을 문장처럼 드러내고, 가능한 한 테스트 하나가 하나의 행동만 검증하게 한다.

## 10. 성능과 동시성 기준

- 서버는 async I/O 기반으로 구현하고 blocking 작업을 핸들러 안에 두지 않는다.
- MongoDB client는 clone 비용이 낮은 shared handle로 관리한다.
- 접근 카운트와 마지막 접근 일자 갱신은 `$inc`, `$set` 기반 원자 업데이트를 사용한다.
- 최근 접근 캐시는 프로세스 메모리 안에서 bounded cache로 유지하며, 삭제된 hash는 즉시 무효화한다.
- 불필요한 `String` 복제와 중간 collection 생성을 피한다.
- 성능 최적화는 추측으로 진행하지 않고, 필요 시 `cargo bench` 또는 release 빌드 기반 부하 테스트 결과를 보고 진행한다.

## 11. 품질 도구와 CI 기준

- `cargo fmt`를 통과해야 한다.
- `cargo clippy --all-targets --all-features --locked -- -D warnings`를 통과해야 한다.
- `cargo test --all-features --locked`를 통과해야 한다.
- `Cargo.toml`에 lint 정책을 추가해 기본 Rust/Clippy 경고를 조기에 잡는다.
- 공개 API에는 필요한 doc comment를 작성하고, 구현 세부사항을 설명하는 장황한 주석은 피한다.

## 12. Docker와 실행 문서

- multi-stage `Dockerfile`을 작성한다.
  - builder stage는 Alpine 기반 Rust 이미지에서 musl release binary를 빌드한다.
  - runtime stage는 `scratch`를 사용하고 실행 바이너리와 CA 번들만 포함한다.
  - 컨테이너는 숫자 UID/GID 기반 non-root 사용자로 실행한다.
- `.dockerignore`를 작성한다.
- Docker Compose 기반 로컬 실행 구성을 작성한다.
- `.env.sample`을 작성한다.
- 컨테이너 이미지 게시 워크플로는 `v*` 태그 push에서만 실행한다.
- 로컬 실행 예시를 `README.md`에 추가한다.
  - 환경변수 예시
  - MongoDB 실행 방법
  - API 호출 예시
  - Docker build/run 예시

## 13. 구현 순서

1. Cargo 프로젝트와 기본 모듈 구조를 만든다.
2. `config`, `error`, `model`의 핵심 타입을 정의한다.
3. MongoDB 연결과 index 초기화 코드를 작성한다.
4. 해시 생성, URL 검증, TTL 계산 로직을 구현하고 단위 테스트를 붙인다.
5. 인증 extractor 또는 middleware를 구현하고 테스트한다.
6. `POST /gen`과 저장소 insert 경로를 구현한다.
7. `GET /{hash}` 리다이렉트와 접근 통계 갱신을 구현한다.
8. `DELETE /{hash}`를 구현한다.
9. `GET /stat`를 구현한다.
10. 통합 테스트를 추가한다.
11. Dockerfile, Docker Compose, `.env.sample`, README를 작성한다.
12. 릴리스 태그에서만 도커 이미지를 빌드하도록 CI 워크플로를 관리한다.
13. `fmt`, `clippy`, `test`, release build를 실행해 마무리 검증한다.

## 14. 미정 사항

- TTL 입력은 duration 문자열로 확정했다.
- `GET /stat`는 같은 `APP_KEY` 기반 인증을 적용하기로 확정했다.
- 만료된 링크 조회는 `404 Not Found`로 확정했다.
- 생성 요청에서 사용자 지정 hash는 허용하지 않기로 확정했다.

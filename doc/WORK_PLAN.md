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
- 시간 처리는 TTL과 마지막 접근 일자 표현을 위해 `chrono` 또는 `time` 중 하나를 선택한다. MongoDB BSON 연동성을 우선해 최종 선택한다. <- 알아서 골라
- 환경변수 로딩은 운영 환경에서는 직접 환경변수를 읽고, 로컬 개발 편의를 위해 `dotenvy` 사용 여부를 검토한다. <- dotenvy 불필요. .env 환경은 docker-compose 의 지원기능으로 커버. .env.sample 생성할 것.
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
  - 만료된 링크는 `404 Not Found` 또는 `410 Gone` 중 하나로 정책을 확정해 반환한다. -> 404
  - 접근 횟수와 마지막 접근 일자를 원자적으로 갱신한다.
  - 원본 URL로 리다이렉트한다.
- `GET /stat`
  - 생성된 링크들의 원본 링크, 접근 횟수, 마지막 접근 일자를 반환한다.
  - 초기 버전은 전체 목록을 반환하되, 데이터가 늘어나는 상황을 고려해 pagination 추가 지점을 남긴다.

## 7. 인증 정책

- `POST /gen`, `DELETE /{hash}`는 `Authorization` 헤더가 `APP_KEY`와 일치해야 한다.
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
- MongoDB가 필요한 테스트는 `testcontainers` 도입을 검토한다.
- 테스트 이름은 동작과 조건을 문장처럼 드러내고, 가능한 한 테스트 하나가 하나의 행동만 검증하게 한다.

## 10. 성능과 동시성 기준

- 서버는 async I/O 기반으로 구현하고 blocking 작업을 핸들러 안에 두지 않는다.
- MongoDB client는 clone 비용이 낮은 shared handle로 관리한다.
- 접근 카운트와 마지막 접근 일자 갱신은 `$inc`, `$set` 기반 원자 업데이트를 사용한다.
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
  - builder stage에서 release binary를 빌드한다.
  - runtime stage에는 실행에 필요한 최소 파일만 포함한다.
- `.dockerignore`를 작성한다.
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
11. Dockerfile과 README를 작성한다.
12. `fmt`, `clippy`, `test`, release build를 실행해 마무리 검증한다.

## 14. 미정 사항

- TTL 입력 단위는 seconds, RFC3339 만료 시각, duration 문자열 중 하나로 확정해야 한다. -> duration 문자열
- `GET /stat` 인증 여부는 현재 기획에 없으므로 공개 API로 둘지, 운영 정보 보호를 위해 인증을 추가할지 결정해야 한다. -> 같은 키를 사용해 인증
- 만료된 링크 조회 시 `404 Not Found`와 `410 Gone` 중 어느 상태 코드를 사용할지 결정해야 한다. -> 404
- 생성 요청에서 사용자 지정 hash를 허용할지 여부를 결정해야 한다. -> 불허

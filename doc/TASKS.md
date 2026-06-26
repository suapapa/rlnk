# rlnk 작업 체크리스트

여러 코딩 에이전트가 동시에 작업할 수 있도록 작업을 작은 단위로 나눈다. 작업을 시작하는 에이전트는 해당 항목의 `Owner`를 자신의 식별자로 바꾸고, 완료하면 체크박스를 `[x]`로 바꾼다.

## 사용 규칙

- `Owner: -`인 항목만 새로 맡는다.
- 작업 중인 항목은 `Owner: <agent-id>`로 표시한다.
- 완료한 항목은 `[x]`로 체크하고, 필요한 경우 `Note:`에 검증 명령이나 남은 이슈를 적는다.
- 막힌 항목은 체크하지 말고 `Note: blocked - ...`로 이유를 남긴다.
- 이미 `Owner`가 있는 항목의 파일 범위는 먼저 확인하고 겹치면 다른 항목을 맡는다.

## 0. 협업 준비

- [x] T00-01 Cargo 프로젝트 초기화 여부 확인 및 생성. Owner: codex; Files: `Cargo.toml`, `Cargo.lock`, `src/main.rs`; Note: `cargo init --bin --name rlnk .`
- [x] T00-02 기본 모듈 구조 생성. Owner: codex; Files: `src/lib.rs`, `src/config.rs`, `src/error.rs`, `src/model.rs`, `src/auth.rs`, `src/store.rs`, `src/http.rs`
- [x] T00-03 공통 lint와 formatting 정책 추가. Owner: codex; Files: `Cargo.toml`, `rustfmt.toml`

## 1. 설정과 오류 모델

- [x] T01-01 환경변수 로딩 구현. Owner: codex; Files: `src/config.rs`
- [x] T01-02 설정 기본값과 필수값 검증 테스트 추가. Owner: codex; Files: `src/config.rs`
- [x] T01-03 애플리케이션 오류 enum 구현. Owner: codex; Files: `src/error.rs`
- [x] T01-04 HTTP 오류 응답 JSON 포맷과 상태 코드 매핑 구현. Owner: codex; Files: `src/error.rs`, `src/http.rs`

## 2. 도메인 모델과 검증

- [x] T02-01 링크 도메인 타입과 DB 문서 타입 정의. Owner: codex; Files: `src/model.rs`
- [x] T02-02 URL 검증 구현. Owner: codex; Files: `src/model.rs` 또는 `src/validation.rs`
- [x] T02-03 duration 문자열 TTL 파싱과 만료 시각 계산 구현. Owner: codex; Files: `src/model.rs` 또는 `src/time.rs`
- [x] T02-04 모델과 검증 로직 단위 테스트 추가. Owner: codex; Files: `src/model.rs`, 관련 테스트 파일

## 3. 해시 생성

- [x] T03-01 URL-safe 해시 생성기 구현. Owner: codex; Files: `src/hash.rs`, `src/lib.rs`
- [x] T03-02 해시 길이와 alphabet 단위 테스트 추가. Owner: codex; Files: `src/hash.rs`
- [x] T03-03 충돌 재시도 정책을 저장소 insert 흐름과 연결. Owner: codex; Files: `src/store.rs`, `src/http.rs`

## 4. MongoDB 저장소

- [x] T04-01 MongoDB client와 collection 초기화 구현. Owner: codex; Files: `src/store.rs`
- [x] T04-02 `hash` unique index와 `expires_at` TTL index 생성 구현. Owner: codex; Files: `src/store.rs`
- [x] T04-03 링크 생성 insert 구현. Owner: codex; Files: `src/store.rs`
- [x] T04-04 링크 조회와 원자적 접근 통계 갱신 구현. Owner: codex; Files: `src/store.rs`
- [x] T04-05 링크 삭제 구현. Owner: codex; Files: `src/store.rs`
- [x] T04-06 통계 목록 조회 구현. Owner: codex; Files: `src/store.rs`

## 5. 인증

- [x] T05-01 `Authorization` 헤더 검증 구현. Owner: codex; Files: `src/auth.rs`
- [x] T05-02 인증 실패 응답 테스트 추가. Owner: codex; Files: `src/auth.rs`, `src/http.rs`
- [x] T05-03 보호 대상 라우트에 인증 적용. Owner: codex; Files: `src/http.rs`

## 6. HTTP API

- [x] T06-01 application state와 router 생성 함수 구현. Owner: codex; Files: `src/http.rs`, `src/lib.rs`
- [x] T06-02 `POST /gen` handler 구현. Owner: codex; Files: `src/http.rs`
- [x] T06-03 `GET /{hash}` redirect handler 구현. Owner: codex; Files: `src/http.rs`
- [x] T06-04 `DELETE /{hash}` handler 구현. Owner: codex; Files: `src/http.rs`
- [x] T06-05 `GET /stat` handler 구현. Owner: codex; Files: `src/http.rs`
- [x] T06-06 라우팅 순서 검증. `GET /stat`는 `/{hash}`보다 먼저 등록한다. Owner: codex; Files: `src/http.rs`

## 7. 실행 진입점과 관측성

- [x] T07-01 `main`에서 설정 로딩, MongoDB 연결, index 초기화, 서버 실행 구현. Owner: codex; Files: `src/main.rs`
- [x] T07-02 `tracing` 초기화와 기본 request logging 적용. Owner: codex; Files: `src/main.rs`, `src/http.rs`
- [x] T07-03 graceful shutdown 구현. Owner: codex; Files: `src/main.rs`

## 8. 통합 테스트

- [x] T08-01 HTTP handler 테스트용 app factory 정리. Owner: codex; Files: `src/http.rs`, `tests/api.rs`
- [x] T08-02 `POST /gen` 성공과 인증 실패 통합 테스트. Owner: codex; Files: `tests/api.rs`
- [x] T08-03 `GET /{hash}` 리다이렉트와 접근 카운트 증가 통합 테스트. Owner: codex; Files: `tests/api.rs`
- [x] T08-04 `DELETE /{hash}` 삭제 후 조회 실패 통합 테스트. Owner: codex; Files: `tests/api.rs`
- [x] T08-05 `GET /stat` 인증 실패와 응답 구조 통합 테스트. Owner: codex; Files: `tests/api.rs`
- [x] T08-06 저장소 테스트 전략 확정. Owner: codex; Files: `src/store.rs`, `tests/api.rs`; Note: handler integration은 `MemoryLinkStore` 기반으로 수행하고, 실제 Mongo 연동 확인은 T12-02 smoke test로 남긴다.

## 9. Docker와 로컬 실행

- [x] T09-01 multi-stage `Dockerfile` 작성. Owner: codex; Files: `Dockerfile`
- [x] T09-02 `.dockerignore` 작성. Owner: codex; Files: `.dockerignore`
- [x] T09-03 Docker Compose 로컬 MongoDB와 앱 실행 구성 작성. Owner: codex; Files: `docker-compose.yml`
- [x] T09-04 `.env.sample` 작성. Owner: codex; Files: `.env.sample`

## 10. 사용자 문서

- [x] T10-01 README 기본 설명과 실행 방법 작성. Owner: codex; Files: `README.md`
- [x] T10-02 API 요청/응답 예시 작성. Owner: codex; Files: `README.md`
- [x] T10-03 운영 환경변수 설명 작성. Owner: codex; Files: `README.md`

## 11. 품질 검증

- [x] T11-01 `cargo fmt` 통과. Owner: codex; Files: 전체
- [x] T11-02 `cargo clippy --all-targets --all-features --locked -- -D warnings` 통과. Owner: codex; Files: 전체
- [x] T11-03 `cargo test --all-features --locked` 통과. Owner: codex; Files: 전체
- [x] T11-04 `cargo build --release --locked` 통과. Owner: codex; Files: 전체

## 12. 릴리스 전 점검

- [x] T12-01 `doc/PLANNING.md`, `doc/WORK_PLAN.md`, `doc/TASKS.md` 내용이 구현과 일치하는지 확인. Owner: codex; Files: `doc/*.md`
- [x] T12-02 실제 실행으로 URL 생성, 리다이렉트, 삭제, 통계 조회 smoke test 수행. Owner: codex; Files: 전체; Note: `docker compose up -d --build` 후 `POST /gen`, `GET /{hash}`, `GET /stat`, `DELETE /{hash}`, 삭제 후 재조회 `404` 확인
- [x] T12-03 남은 `Owner`와 `blocked` 항목 정리. Owner: codex; Files: `doc/TASKS.md`

## 13. 이미지 최적화

- [x] T13-01 Docker runtime 이미지를 `scratch`로 축소. Owner: codex; Files: `Dockerfile`, `doc/WORK_PLAN.md`, `doc/TASKS.md`; Note: `docker build -t rlnk:scratch .` 및 `docker compose build app` 통과, 최종 이미지 `10.6MB`; `docker run --rm rlnk-app:latest`가 설정 오류까지 정상 실행됨

# AGENTS.md

이 저장소는 여러 코딩 에이전트가 동시에 작업할 수 있다. 작업 전 이 파일과 `doc/PLANNING.md`, `doc/WORK_PLAN.md`, `doc/TASKS.md`를 먼저 읽는다.

## 프로젝트 개요

- Rust로 작성하는 초소형 고성능 URL shortener API다.
- HTTP 서버는 `axum`, async runtime은 `tokio`, 데이터베이스는 MongoDB를 사용한다.
- 주요 API는 `POST /gen`, `DELETE /{hash}`, `GET /{hash}`, `GET /stat`이다.
- `POST /gen`, `DELETE /{hash}`, `GET /stat`는 `Authorization` 헤더 값이 `APP_KEY`와 일치해야 한다.
- TTL은 `10m`, `24h`, `7d` 같은 duration 문자열로 받는다. 생략하면 만료 없음이다.
- 만료되거나 존재하지 않는 hash 조회는 `404 Not Found`로 응답한다.

## 작업 분담 절차

1. `doc/TASKS.md`에서 `Owner: -`인 항목을 하나 고른다.
2. 작업 시작 전에 해당 항목의 `Owner`를 자신의 식별자로 바꾼다.
3. 같은 파일을 만질 가능성이 있는 다른 `Owner` 항목이 있으면 작업 범위를 줄이거나 다른 항목을 고른다.
4. 작업 완료 후 체크박스를 `[x]`로 바꾸고 필요한 검증 결과를 `Note:`에 적는다.
5. 작업이 막히면 체크하지 않고 `Note: blocked - <이유>`를 남긴다.

## 동시 작업 규칙

- 맡은 작업과 관련 없는 파일은 수정하지 않는다.
- 편집 직전 대상 파일을 다시 읽어 다른 에이전트의 변경을 확인한다.
- 이미 존재하는 사용자 또는 다른 에이전트의 변경을 되돌리지 않는다.
- 큰 리팩터링은 여러 작업을 한꺼번에 고치지 말고 별도 체크리스트 항목으로 분리한다.
- 공통 파일인 `Cargo.toml`, `src/lib.rs`, `src/http.rs`, `src/store.rs`를 수정할 때는 `doc/TASKS.md`의 다른 소유 작업과 겹치는지 먼저 확인한다.
- 파일 포맷만 바뀌는 변경과 기능 변경을 한 작업에 섞지 않는다.

## Rust 구현 기준

- 생산 코드에서 `unwrap()`과 `expect()`를 사용하지 않는다.
- 오류는 `thiserror` 기반의 명시적 enum으로 모델링한다.
- 핸들러는 가능한 한 `Result<T, AppError>` 형태로 작성하고, HTTP 응답 변환은 한 곳에 모은다.
- 함수 인자는 소유권이 필요하지 않으면 `&str`, `&T`, `&[T]`를 우선 사용한다.
- 불필요한 `String` 복제, 중간 `Vec` 수집, blocking I/O를 피한다.
- MongoDB 접근은 async API를 사용하고, 접근 횟수 갱신은 `$inc`, 마지막 접근 일자는 `$set`으로 원자적으로 처리한다.
- 공개 API와 모듈에는 필요한 doc comment를 작성하되, 구현을 그대로 설명하는 주석은 남기지 않는다.

## 검증 명령

구현 작업 후 가능한 범위에서 아래 명령을 실행한다.

```sh
cargo fmt
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-features --locked
cargo build --release --locked
```

아직 Cargo 프로젝트가 없거나 MongoDB 의존 테스트 환경이 준비되지 않은 경우, 실행하지 못한 명령과 이유를 작업 항목의 `Note:`에 남긴다.

## 문서 정책

- 요구사항 변경은 먼저 `doc/PLANNING.md`에 반영한다.
- 구현 전략 변경은 `doc/WORK_PLAN.md`에 반영한다.
- 실제 진행 상태는 `doc/TASKS.md`에 반영한다.
- 에이전트 협업 규칙 변경은 이 파일에 반영한다.

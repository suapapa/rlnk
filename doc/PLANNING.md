프로그램 개발 계획.

Rust로 작성된 초소형 고성능 URL shortener 앱.

- 데이터베이스로 MongoDB를 사용
- `POST /gen`: URL 생성. 원본 링크와 선택 TTL을 전달한다.
  - TTL은 `10m`, `24h`, `7d` 같은 duration 문자열로 받는다.
  - TTL을 생략하면 만료 없음으로 저장한다.
  - 사용자 지정 hash는 허용하지 않고 서버가 생성한다.
- `DELETE /{hash}`: 생성된 URL 삭제
- `GET /{hash}`: 생성된 URL로 리다이렉트
  - 존재하지 않거나 만료된 hash는 `404 Not Found`로 반환한다.
- `GET /stat`: 생성된 링크들의 원본 링크, 접근 횟수, 마지막 접근 일자 반환
- `POST /gen`, `DELETE /{hash}`, `GET /stat`는 `Authorization` 헤더로 보안 강화
  - `Authorization: Bearer <APP_KEY>` 형식으로 인증한다.
- Dockerfile 필요
- Docker Compose 기반 로컬 실행을 지원하고, `.env.sample`을 제공한다.
- 환경변수로 `MONGO_URI`, `APP_KEY`, `APP_HOSTNAME` 등을 받는다.
  - `APP_HOSTNAME`은 생성한 hash에 붙여 short URL로 반환한다.

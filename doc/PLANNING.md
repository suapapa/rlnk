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
  - 최근 접근한 hash의 원본 URL과 만료 시각은 메모리에 캐시해서 반복 접근 시 DB에서 문서를 다시 읽지 않도록 한다.
  - 접근 횟수/마지막 접근 일자는 write-behind로 버퍼링한 뒤 주기적으로 MongoDB에 flush한다. `GET /stat` 직전에 flush해서 관리 API는 최신 통계를 본다.
- `GET /stat`: 생성된 링크들의 원본 링크, 접근 횟수, 마지막 접근 일자 반환
  - 선택 쿼리 파라미터로 `limit`(기본값 50, 최대 1000)과 `offset`(기본값 0)을 지원한다.
  - `{ items: [...], total, limit, offset }` 페이징 봉투 형식으로 반환한다.
- `GET /healthz`: 프로세스 활성 상태(liveness) 확인 (인증 불필요, 200 OK)
- `GET /readyz`: MongoDB ping 기반 데이터베이스 준비 상태(readiness) 확인 (인증 불필요, 정상 시 200 OK, 장애 시 503)
- `GET /metrics`: 프로메테우스 텍스트 포맷 메트릭 제공 (인증 불필요)
- `POST /gen`, `DELETE /{hash}`, `GET /stat`는 `Authorization` 헤더로 보안 강화
  - `Authorization: Bearer <APP_KEY>` 형식으로 인증한다.
- Dockerfile 필요
- 도커 이미지는 `v*` 태그가 발행될 때만 CI에서 빌드하고 게시한다.
- Docker Compose 기반 로컬 실행을 지원하고, `.env.sample`을 제공한다.
- 환경변수로 `MONGO_URI`, `APP_KEY`, `APP_HOSTNAME` 등을 받는다.
  - `APP_HOSTNAME`은 생성한 hash에 붙여 short URL로 반환한다.
  - `ACCESS_CACHE_SIZE`는 최근 접근 캐시에 보관할 최대 항목 수이며, 기본값은 `1024`이다. `0`이면 캐시를 비활성화한다.
  - `ACCESS_STATS_FLUSH_INTERVAL_MS`는 접근 통계 write-behind flush 주기(밀리초)이며, 기본값은 `1000`이다.
  - `MONGO_MAX_POOL_SIZE`, `MONGO_CONNECT_TIMEOUT_MS`, `MONGO_SERVER_SELECTION_TIMEOUT_MS`는 선택적 MongoDB 클라이언트 튜닝 값이다.
- 로컬 Compose는 앱 인스턴스 2개와 nginx 로드밸런서로 수평 확장을 지원한다.


# rlnk

<p align="center">
  <img src="assets/rlnk-logo.png" alt="rlnk logo" width="640">
</p>

MongoDB backed URL shortener written in Rust.

## Requirements

- Rust 1.90+
- MongoDB 8+ for local development, or Docker / Docker Compose

## Environment

The application reads the following environment variables:

- `MONGO_URI`: MongoDB connection string
- `APP_KEY`: bearer token secret used by `POST /gen`, `DELETE /{hash}`, and `GET /stat`
- `APP_HOSTNAME`: base URL used to build returned short URLs
- `APP_BIND_ADDR`: optional bind address, defaults to `0.0.0.0:8080`
- `MONGO_DATABASE`: optional database name, defaults to `rlnk`
- `MONGO_COLLECTION`: optional collection name, defaults to `links`
- `HASH_LENGTH`: optional generated hash length, defaults to `8`
- `ACCESS_CACHE_SIZE`: optional number of recently accessed links cached in memory, defaults to `1024`; use `0` to disable
- `ACCESS_STATS_FLUSH_INTERVAL_MS`: how often buffered redirect stats are flushed to MongoDB, defaults to `1000`
- `MONGO_MAX_POOL_SIZE`: optional MongoDB client max pool size
- `MONGO_CONNECT_TIMEOUT_MS`: optional MongoDB connect timeout in milliseconds
- `MONGO_SERVER_SELECTION_TIMEOUT_MS`: optional MongoDB server selection timeout in milliseconds

Redirect access counts are write-behind: `GET /{hash}` records hits in memory and flushes them on an interval (and before `GET /stat`). Stats may lag by up to one flush interval.

## Local run

Start MongoDB locally and export environment variables:

```sh
export MONGO_URI='mongodb://localhost:27017'
export APP_KEY='dev-secret'
export APP_HOSTNAME='http://localhost:8080'
cargo run
```

## Docker Compose

Create a local `.env` file from `.env.sample`, then start the stack (two app replicas behind nginx on port `8080`):

```sh
cp .env.sample .env
docker compose up --build
```

## Load test

With the stack running, measure redirect throughput (install [oha](https://github.com/hatoo/oha) for best results):

```sh
APP_KEY='change-me' ./scripts/loadtest.sh
```

## API

Create a short URL:

```sh
curl -X POST http://localhost:8080/gen \
  -H 'Authorization: Bearer dev-secret' \
  -H 'Content-Type: application/json' \
  -d '{"url":"https://example.com","ttl":"10m"}'
```

Delete a short URL:

```sh
curl -X DELETE http://localhost:8080/abc123 \
  -H 'Authorization: Bearer dev-secret'
```

Fetch statistics (supports optional `limit` and `offset` query parameters):

```sh
curl 'http://localhost:8080/stat?limit=50&offset=0' \
  -H 'Authorization: Bearer dev-secret'
```

Response format:

```json
{
  "items": [
    {
      "hash": "abc12345",
      "short_url": "http://localhost:8080/abc12345",
      "original_url": "https://example.com",
      "access_count": 1,
      "created_at": "2026-10-05T01:00:00Z",
      "expires_at": "2026-10-05T01:10:00Z",
      "last_accessed_at": "2026-10-05T01:05:00Z"
    }
  ],
  "total": 1,
  "limit": 50,
  "offset": 0
}
```

Resolve a short URL:

```sh
curl -i http://localhost:8080/abc123
```

Check service liveness:

```sh
curl -i http://localhost:8080/healthz
```

Check database readiness:

```sh
curl -i http://localhost:8080/readyz
```

Scrape Prometheus metrics:

```sh
curl http://localhost:8080/metrics
```


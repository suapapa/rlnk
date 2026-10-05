#!/usr/bin/env sh
# Baseline redirect load test against a running rlnk instance.
# Requires: curl. Prefer oha (https://github.com/hatoo/oha) when available.
set -eu

BASE_URL="${BASE_URL:-http://localhost:8080}"
APP_KEY="${APP_KEY:-change-me}"
DURATION="${DURATION:-10s}"
CONCURRENCY="${CONCURRENCY:-50}"

echo "Creating a short link against ${BASE_URL}..."
CREATE_JSON="$(curl -fsS -X POST "${BASE_URL}/gen" \
  -H "Authorization: Bearer ${APP_KEY}" \
  -H "Content-Type: application/json" \
  -d '{"url":"https://example.com/loadtest"}')"

HASH="$(printf '%s' "${CREATE_JSON}" | sed -n 's/.*"hash":"\([^"]*\)".*/\1/p')"
if [ -z "${HASH}" ]; then
  echo "failed to parse hash from: ${CREATE_JSON}" >&2
  exit 1
fi

TARGET="${BASE_URL}/${HASH}"
echo "Warming cache with one redirect to ${TARGET}..."
curl -fsS -o /dev/null -w "%{http_code}\n" "${TARGET}" >/dev/null

echo "Running redirect load test (duration=${DURATION}, concurrency=${CONCURRENCY})..."
if command -v oha >/dev/null 2>&1; then
  oha -z "${DURATION}" -c "${CONCURRENCY}" --no-tui "${TARGET}"
else
  echo "oha not found; falling back to sequential curl samples" >&2
  i=0
  while [ "${i}" -lt 100 ]; do
    curl -fsS -o /dev/null -w "%{time_total}\n" "${TARGET}"
    i=$((i + 1))
  done
fi

echo "Metrics snapshot:"
curl -fsS "${BASE_URL}/metrics" | grep -E 'rlnk_(redirects|cache_hits|cache_misses)_total' || true

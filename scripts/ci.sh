#!/usr/bin/env bash
# Local CI simulation — container-first, fail-fast (rules §6.5, §8.6).
set -euo pipefail
cd "$(dirname "$0")/.."

[ -f .env ] || cp .env.example .env
# shellcheck disable=SC1091
source .env
DBURL="postgres://${POSTGRES_USER}:${POSTGRES_PASSWORD}@localhost:${POSTGRES_PORT}/${POSTGRES_DB}"

stage() { echo; echo "==================== $* ===================="; }

stage "db up"
docker compose up -d db
for _ in $(seq 1 30); do
  docker compose exec -T db pg_isready -U "${POSTGRES_USER}" -d "${POSTGRES_DB}" >/dev/null 2>&1 && break
  sleep 2
done

stage "fmt"
(cd api && cargo fmt --check)

stage "clippy -D warnings"
(cd api && cargo clippy --all-targets -- -D warnings)

stage "unit + integration tests"
(cd api && DATABASE_URL="${DBURL}" cargo test)

stage "coverage (lines must be 100%)"
(cd api && DATABASE_URL="${DBURL}" cargo llvm-cov --ignore-filename-regex 'src/main\.rs' --fail-under-lines 100)

stage "admin typecheck"
# The admin UI carries the Stripe connection screen — the one place an operator can put a
# live key in the wrong mode. Until now nothing built or typechecked it in CI, so a broken
# import or a field that no longer exists surfaced only when someone opened the page.
# `next lint` is deliberately NOT here: admin/ has no ESLint config, and without one `next lint`
# opens an interactive "how would you like to configure ESLint?" prompt — a CI job that never
# ends. Add the config first (a separate task), then the lint stage.
(cd admin && npm ci --no-audit --no-fund && npx tsc --noEmit)

stage "image build"
docker compose build api

stage "container e2e (curl)"
docker compose up -d
for _ in $(seq 1 40); do
  curl -fsS "localhost:${API_PORT}/health" >/dev/null 2>&1 && break
  sleep 2
done
echo -n "/health -> "; curl -fsS "localhost:${API_PORT}/health"; echo
echo -n "/api/events -> "; curl -fsS "localhost:${API_PORT}/api/events"; echo

stage "teardown"
docker compose down

echo; echo "CI OK"

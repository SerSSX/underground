#!/usr/bin/env bash
# Apply the dev content seed to the running compose DB (idempotent).
set -euo pipefail
cd "$(dirname "$0")/.."

[ -f .env ] || cp .env.example .env
# shellcheck disable=SC1091
source .env

docker compose up -d db >/dev/null
PGPASSWORD="${POSTGRES_PASSWORD}" psql -h localhost -p "${POSTGRES_PORT}" \
  -U "${POSTGRES_USER}" -d "${POSTGRES_DB}" -v ON_ERROR_STOP=1 -q \
  -f api/seeds/dev_events.sql

count=$(PGPASSWORD="${POSTGRES_PASSWORD}" psql -h localhost -p "${POSTGRES_PORT}" \
  -U "${POSTGRES_USER}" -d "${POSTGRES_DB}" -tAc "SELECT count(*) FROM events;")
echo "seeded — events in DB: ${count}"

#!/usr/bin/env bash
# Seed test accounts via the auth API (idempotent: 409 = already exists), then
# make the maker the organizer of seeded events. Password for both: "underground".
set -euo pipefail
cd "$(dirname "$0")/.."

[ -f .env ] || cp .env.example .env
# shellcheck disable=SC1091
source .env
API="localhost:${API_PORT}"

docker compose up -d >/dev/null
for _ in $(seq 1 30); do curl -fsS "${API}/health" >/dev/null 2>&1 && break; sleep 2; done

register() { # email name
  local code
  code=$(curl -s -o /dev/null -w "%{http_code}" -X POST "${API}/api/auth/register" \
    -H 'content-type: application/json' \
    -d "{\"email\":\"$1\",\"password\":\"underground\",\"name\":\"$2\"}")
  echo "  $1 → HTTP ${code}"
}

echo "seeding test accounts (password: underground):"
register "user@underground.dev"  "Тестовий Користувач"
register "maker@underground.dev" "Underground Maker"
register "admin@underground.dev" "Underground Admin"

PGPASSWORD="${POSTGRES_PASSWORD}" psql -h localhost -p "${POSTGRES_PORT}" \
  -U "${POSTGRES_USER}" -d "${POSTGRES_DB}" -q \
  -c "UPDATE events SET organizer_id = (SELECT id FROM users WHERE email='maker@underground.dev') WHERE organizer_id IS NULL;" \
  -c "UPDATE users SET role='ADMIN' WHERE email='admin@underground.dev';"
echo "maker@underground.dev → organizer of seeded events; admin@underground.dev → ADMIN role"

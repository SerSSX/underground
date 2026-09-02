#!/usr/bin/env bash
# Ship the working copy to the Underground DEV stand. Idempotent; safe to re-run.
# Usage: bash deploy/publish-dev.sh [--no-build]
set -euo pipefail
cd "$(dirname "$0")/.."

HOST=${UG_DEV_HOST:-root@159.69.159.230}
KEY=${UG_DEV_SSH_KEY:-$HOME/.ssh/hetzner}
ROOT=${UG_DEV_ROOT:-/opt/underground-dev}
WEB_URL=${UG_DEV_WEB_URL:-https://underground.159-69-159-230.sslip.io}
ADMIN_URL=${UG_DEV_ADMIN_URL:-https://underground-admin.159-69-159-230.sslip.io}
SSH="ssh -i $KEY $HOST"

echo "→ rsync sources to $HOST:$ROOT/src"
rsync -az --delete \
  --exclude '.git' --exclude 'node_modules' --exclude 'target' --exclude '.next' \
  --exclude '.dev-data' --exclude '.env' --exclude '*.tsbuildinfo' --exclude 'android' \
  -e "ssh -i $KEY" ./ "$HOST:$ROOT/src/"

if [ "${1:-}" = "--no-build" ]; then echo "→ skipping build (--no-build)"; exit 0; fi

echo "→ build + up"
$SSH "cd $ROOT/src && docker compose --env-file $ROOT/.env.dev -f deploy/docker-compose.dev.yml up -d --build"

echo "→ smoke"
fail=0
check() { # url expected-code label
  code=$(curl -s -o /dev/null -w '%{http_code}' --max-time 30 "$1")
  [ "$code" = "$2" ] && echo "  ok   $3 ($code)" || { echo "  FAIL $3 (got $code, want $2)"; fail=1; }
}
check "$WEB_URL/health"       200 "api health"
check "$WEB_URL/api/version"  200 "api version"
check "$WEB_URL/"             200 "web home"
check "$ADMIN_URL/"           200 "admin home"
[ "$fail" = 0 ] || { echo "smoke FAILED"; exit 1; }
echo "→ done: $WEB_URL"

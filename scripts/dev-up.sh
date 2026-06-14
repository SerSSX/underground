#!/usr/bin/env bash
# One-command dev up with automatic API port conflict resolution (rules §8.1).
set -euo pipefail
cd "$(dirname "$0")/.."

[ -f .env ] || cp .env.example .env

# shellcheck disable=SC1091
source .env

port_busy() { lsof -iTCP:"$1" -sTCP:LISTEN >/dev/null 2>&1; }

if port_busy "${API_PORT}"; then
  for p in $(seq $((API_PORT + 1)) $((API_PORT + 20))); do
    if ! port_busy "$p"; then
      sed -i.bak "s/^API_PORT=.*/API_PORT=$p/" .env && rm -f .env.bak
      echo "API_PORT ${API_PORT} busy → using $p"
      break
    fi
  done
fi

exec docker compose up --build "$@"

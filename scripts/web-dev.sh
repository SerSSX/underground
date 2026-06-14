#!/usr/bin/env bash
# Public web client (afisha + tickets + organizer cabinet) on :3001.
# Talks to the dev API on :8080 (NEXT_PUBLIC_API_BASE_URL).
set -euo pipefail
cd "$(dirname "$0")/../web"
[ -d node_modules ] || npm install
[ -f .env ] || cp .env.example .env
exec npm run dev

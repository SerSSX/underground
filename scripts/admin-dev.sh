#!/usr/bin/env bash
# Run the Next.js admin dev server (port 3000). Talks to the API at NEXT_PUBLIC_API_BASE_URL.
set -euo pipefail
cd "$(dirname "$0")/../admin"

[ -f .env ] || cp .env.example .env
[ -d node_modules ] || npm install

npm run dev

#!/usr/bin/env bash
# Replace the DEV stand's content with the demo dataset. Idempotent; safe to re-run.
# Usage: bash scripts/seed-demo.sh
#
# Wipes events (and everything that cascades off them) plus the probe accounts left by
# security runs, then inserts the dataset in ONE transaction — so a mid-way failure leaves
# the stand on its old content rather than half-seeded.
set -euo pipefail
cd "$(dirname "$0")/.."

HOST=${UG_DEV_HOST:-root@159.69.159.230}
KEY=${UG_DEV_SSH_KEY:-$HOME/.ssh/hetzner}
ROOT=${UG_DEV_ROOT:-/opt/underground-dev}
WEB_URL=${UG_DEV_WEB_URL:-https://underground.159-69-159-230.sslip.io}
SSH="ssh -i $KEY $HOST"
STAMP=$(date +%F-%H%M%S)

command -v node >/dev/null || { echo "node is required to build the seed"; exit 1; }

echo "→ build SQL from api/seeds/demo.json"
node api/seeds/build-seed.mjs api/seeds/demo.json > "/tmp/underground-seed-$STAMP.sql"
echo "  $(wc -l < "/tmp/underground-seed-$STAMP.sql") lines"

# A backup BEFORE a destructive step, every time: this deletes real rows, and an
# untested backup is not a backup — restore is `gunzip -c … | psql`.
echo "→ pg_dump backup on the box"
$SSH "docker exec ${UG_DB:-underground-dev-db} pg_dump -U underground -d underground_dev \
  | gzip > /opt/backups/underground-dev-pre-seed-$STAMP.sql.gz && \
  ls -lh /opt/backups/underground-dev-pre-seed-$STAMP.sql.gz"

echo "→ apply"
# ON_ERROR_STOP so a failing statement aborts the transaction instead of committing a
# partially-applied dataset that looks fine until someone opens the page it broke.
$SSH "docker exec -i ${UG_DB:-underground-dev-db} psql -U underground -d underground_dev \
  -v ON_ERROR_STOP=1 -q" < "/tmp/underground-seed-$STAMP.sql"

# Next.js caches GET /api/events for 300s on the SSR home page, so without this the owner
# opens the site and sees the old catalogue and reasonably concludes the seed did nothing.
echo "→ restart web to drop the SSR cache"
$SSH "docker restart underground-dev-web >/dev/null && sleep 6"

echo "→ smoke"
code=$(curl -s -o /dev/null -w '%{http_code}' --max-time 30 "$WEB_URL/")
n=$(curl -s --max-time 30 "$WEB_URL/api/events" | grep -o '"id"' | wc -l | tr -d ' ')
echo "  home HTTP $code · /api/events returns $n events"
[ "$code" = "200" ] && [ "$n" -gt 0 ] || { echo "smoke FAILED"; exit 1; }
echo "→ done: $WEB_URL"

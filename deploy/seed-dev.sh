#!/usr/bin/env bash
# Put the demo content on the DEV stand: the three standing accounts, then the demo dataset
# (api/seeds/demo.json: 32 profiles, 76 events, tickets, follows). Idempotent; safe to re-run.
#
#   bash deploy/seed-dev.sh
#
# The account password is UG_SEED_PASSWORD from .env.dev and is used on the box only. The dataset step WIPES
# the stand's events (cascade) after a pg_dump to /opt/backups, and applies in one transaction.
set -euo pipefail
cd "$(dirname "$0")/.."
HOST=${UG_DEV_HOST:-root@65.108.87.247}
KEY=${UG_DEV_SSH_KEY:-$HOME/.ssh/hetzner}
ssh_box() { ssh -i "$KEY" -o BatchMode=yes -o ConnectTimeout=15 "$HOST" "$@"; }
command -v node >/dev/null || { echo "node is required to build the seed"; exit 1; }
SQL=$(mktemp); trap 'rm -f "$SQL"' EXIT

echo "→ standing accounts"
ssh_box "bash -s" <<'REMOTE'
set -euo pipefail
set -a; . /opt/underground-dev/.env.dev; set +a
python3 - "$PUBLIC_WEB_URL" <<'PY'
import json, os, sys, urllib.request, urllib.error
for email, name in [("user@underground.dev", "Тестовий Користувач"),
                    ("maker@underground.dev", "Underground Maker"),
                    ("admin@underground.dev", "Underground Admin")]:
    req = urllib.request.Request(sys.argv[1] + "/api/auth/register", method="POST",
        data=json.dumps({"email": email, "password": os.environ["UG_SEED_PASSWORD"], "name": name}).encode(),
        headers={"content-type": "application/json"})
    try:
        code = urllib.request.urlopen(req, timeout=30).status
    except urllib.error.HTTPError as e:
        code = e.code
    print(f"  {email}: HTTP {code}" + (" (exists)" if code == 409 else ""))
    if code not in (200, 201, 409):
        sys.exit(1)
PY
docker exec underground-dev-db psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -q -v ON_ERROR_STOP=1 \
  -c "UPDATE users SET role='ADMIN' WHERE email='admin@underground.dev';"
REMOTE

echo "→ demo dataset"
node api/seeds/build-seed.mjs api/seeds/demo.json > "$SQL"
ssh_box "set -a; . /opt/underground-dev/.env.dev; set +a
  docker exec underground-dev-db pg_dump -U \"\$POSTGRES_USER\" -d \"\$POSTGRES_DB\" \
    | gzip > /opt/backups/underground-dev-pre-seed-\$(date +%F-%H%M%S).sql.gz"
ssh_box "set -a; . /opt/underground-dev/.env.dev; set +a
  docker exec -i underground-dev-db psql -U \"\$POSTGRES_USER\" -d \"\$POSTGRES_DB\" -v ON_ERROR_STOP=1 -q" < "$SQL"
# Next.js caches GET /api/events on the home page for 300 s; without a restart the new catalogue shows late.
ssh_box "docker restart underground-dev-web >/dev/null"
echo "→ seeded"

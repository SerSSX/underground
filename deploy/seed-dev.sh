#!/usr/bin/env bash
# Put the demo content on the DEV stand: the three standing accounts, then the demo dataset
# (api/seeds/demo.json: 32 profiles, 76 events, tickets, follows). Idempotent; safe to re-run.
#
#   bash deploy/seed-dev.sh
#
# Passwords come from .env.dev on the box and never leave it: UG_SEED_PASSWORD for user@, maker@ and the demo
# profiles, UG_ADMIN_PASSWORD for admin@ (a tester given the demo login does not get the admin role).
# Registration on DEV is open, so someone could register a standing address first. Before granting ADMIN or
# copying user@'s password hash into the demo profiles, the script logs in to all three accounts with the
# box-only passwords and stops if any login fails: a squatted account is never promoted or copied.
# The dataset step WIPES the stand's events (cascade) after a non-empty pg_dump to /opt/backups.
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
base = sys.argv[1]
accounts = [("user@underground.dev", "Тестовий Користувач", "UG_SEED_PASSWORD"),
            ("maker@underground.dev", "Underground Maker", "UG_SEED_PASSWORD"),
            ("admin@underground.dev", "Underground Admin", "UG_ADMIN_PASSWORD")]
def post(path, body):
    req = urllib.request.Request(base + path, method="POST", data=json.dumps(body).encode(),
                                 headers={"content-type": "application/json"})
    try:
        return urllib.request.urlopen(req, timeout=30).status
    except urllib.error.HTTPError as e:
        return e.code
for email, name, var in accounts:
    code = post("/api/auth/register", {"email": email, "password": os.environ[var], "name": name})
    print(f"  register {email}: HTTP {code}" + (" (exists)" if code == 409 else ""))
    if code not in (200, 201, 409):
        sys.exit(1)
for email, _, var in accounts:
    code = post("/api/auth/login", {"email": email, "password": os.environ[var]})
    if code != 200:
        print(f"STOP: {email} does not accept the box password (HTTP {code}). Someone else may own it;"
              " delete that account on the box before seeding.")
        sys.exit(1)
print("  all three accounts accept the box passwords")
PY
docker exec underground-dev-db psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -q -v ON_ERROR_STOP=1 \
  -c "UPDATE users SET role='ADMIN' WHERE lower(email)='admin@underground.dev';"
REMOTE

echo "→ backup before the destructive step"
ssh_box 'set -euo pipefail; set -a; . /opt/underground-dev/.env.dev; set +a
  F=/opt/backups/underground-dev-pre-seed-$(date +%F-%H%M%S).sql.gz
  docker exec underground-dev-db pg_dump -U "$POSTGRES_USER" -d "$POSTGRES_DB" | gzip > "$F"
  [ "$(gzip -dc "$F" | head -c 4096 | wc -c)" -gt 0 ] || { echo "STOP: empty backup $F"; exit 1; }
  echo "  $F"'

echo "→ demo dataset"
node api/seeds/build-seed.mjs api/seeds/demo.json > "$SQL"
ssh_box 'set -a; . /opt/underground-dev/.env.dev; set +a
  docker exec -i underground-dev-db psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -q' < "$SQL"
# Next.js caches GET /api/events on the home page for 300 s; without a restart the new catalogue shows late.
ssh_box "docker restart underground-dev-web >/dev/null"
echo "→ seeded"

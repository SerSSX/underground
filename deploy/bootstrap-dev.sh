#!/usr/bin/env bash
# One-time setup of the Underground DEV stand on the shared box. Idempotent: a second run changes nothing.
#
#   bash deploy/bootstrap-dev.sh
#
# 1. Directories under /opt/underground-dev.
# 2. .env.dev generated ON THE BOX: every CHANGE_ME becomes a fresh `openssl rand -hex` value. No secret is
#    ever printed, copied off the box or committed (rules §9).
# 3. The Caddy site blocks appended to the shared /opt/caddy/Caddyfile, which fronts LIVE products (msgview and
#    others): backup first, append in place (a bind-mounted single file must keep its inode), validate, then
#    RELOAD (not restart); on an invalid config the backup is put back. Then msgview is checked.
set -euo pipefail
cd "$(dirname "$0")/.."
HOST=${UG_DEV_HOST:-root@65.108.87.247}
KEY=${UG_DEV_SSH_KEY:-$HOME/.ssh/hetzner}
ROOT=/opt/underground-dev
ssh_box() { ssh -i "$KEY" -o BatchMode=yes -o ConnectTimeout=15 "$HOST" "$@"; }

ssh_box "mkdir -p $ROOT/data/pg $ROOT/data/uploads $ROOT/incoming && chown -R 10001:10001 $ROOT/data/uploads"
scp -q -i "$KEY" deploy/.env.dev.example deploy/caddy-underground-dev.snippet "$HOST:$ROOT/"

ssh_box "bash -s" <<'REMOTE'
set -euo pipefail
ROOT=/opt/underground-dev; cd "$ROOT"
if [ -f .env.dev ]; then
  echo "→ .env.dev exists, kept"
else
  umask 077
  while IFS= read -r line; do
    case "$line" in *=CHANGE_ME) echo "${line%=CHANGE_ME}=$(openssl rand -hex 24)";; *) echo "$line";; esac
  done < .env.dev.example > .env.dev
  chmod 600 .env.dev
  echo "→ .env.dev generated ($(grep -c . .env.dev) lines, values not shown)"
fi
grep -q '=CHANGE_ME$' .env.dev && { echo "ERROR: .env.dev still has CHANGE_ME"; exit 1; }

CF=/opt/caddy/Caddyfile
if grep -q '^underground-dev\.65-108-87-247\.sslip\.io {' "$CF"; then
  echo "→ Caddy blocks already present, kept"
else
  BK=/opt/backups/Caddyfile.pre-underground-dev-$(date +%F-%H%M%S)
  cp -a "$CF" "$BK"
  { echo; cat caddy-underground-dev.snippet; } >> "$CF"
  if docker exec caddy caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile >/dev/null 2>&1 \
     && docker exec caddy caddy reload --config /etc/caddy/Caddyfile --adapter caddyfile; then
    echo "→ Caddy blocks added and reloaded (backup $BK)"
  else
    cat "$BK" > "$CF"   # in place: same inode as the container's bind mount
    docker exec caddy caddy reload --config /etc/caddy/Caddyfile --adapter caddyfile || true
    echo "ERROR: Caddy rejected the new config; restored $BK"; exit 1
  fi
fi
REMOTE

# The Caddyfile is shared with LIVE msgview: prove msgview still serves its real page, not only a status code.
body=$(curl -fsS --max-time 30 https://msgview.app/) && echo "$body" | grep -qi '<title>[^<]*msgview' \
  && echo "→ msgview.app serves its page after the Caddy reload" \
  || { echo "ERROR: msgview.app check failed after the Caddy reload"; exit 1; }

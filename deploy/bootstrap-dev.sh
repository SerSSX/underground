#!/usr/bin/env bash
# One-time setup of the Underground DEV stand on the shared box. Idempotent: a second run changes nothing.
#
#   bash deploy/bootstrap-dev.sh
#
# 1. A fixed-size filesystem for the stand's data (Postgres + uploads): DEV registration is open, and
#    nothing a DEV user uploads may fill the disk the LIVE products share. 4 GB sparse loop file, nofail mount.
# 2. .env.dev generated ON THE BOX: every CHANGE_ME becomes a fresh `openssl rand -hex` value. No secret is
#    printed, copied off the box or committed (rules §9).
# 3. The Caddy site blocks appended to the shared /opt/caddy/Caddyfile, which fronts LIVE products: backup,
#    append in place (the container bind-mounts the single file, so its inode must not change), validate,
#    RELOAD (not restart). msgview's origin is then checked from the box; on any failure the backup is put
#    back and Caddy reloaded. After that, msgview's own browser smoke (its prod-smoke workflow) must be green,
#    or the Caddy change is undone the same way.
set -euo pipefail
cd "$(dirname "$0")/.."
HOST=${UG_DEV_HOST:-root@65.108.87.247}
KEY=${UG_DEV_SSH_KEY:-$HOME/.ssh/hetzner}
ROOT=/opt/underground-dev
ssh_box() { ssh -i "$KEY" -o BatchMode=yes -o ConnectTimeout=15 "$HOST" "$@"; }

ssh_box "mkdir -p $ROOT/incoming"
scp -q -i "$KEY" deploy/.env.dev.example deploy/caddy-underground-dev.snippet "$HOST:$ROOT/"

LOG=$(mktemp); trap 'rm -f "$LOG"' EXIT
ssh_box "bash -s" <<'REMOTE' | tee "$LOG"
set -euo pipefail
ROOT=/opt/underground-dev; cd "$ROOT"

# 1. Data filesystem
IMG=/opt/underground-dev-data.img
if ! mountpoint -q "$ROOT/data"; then
  [ -z "$(ls -A "$ROOT/data" 2>/dev/null)" ] || { echo "ERROR: $ROOT/data has content but is not mounted"; exit 1; }
  [ -f "$IMG" ] || { truncate -s 4G "$IMG"; mkfs.ext4 -q -F "$IMG"; }
  mkdir -p "$ROOT/data"
  grep -q "^$IMG " /etc/fstab || echo "$IMG $ROOT/data ext4 loop,nofail 0 2" >> /etc/fstab
  mount "$ROOT/data"
  echo "→ $ROOT/data mounted (4 GB cap)"
else
  echo "→ $ROOT/data already mounted"
fi
mkdir -p "$ROOT/data/pg" "$ROOT/data/uploads"
chown -R 10001:10001 "$ROOT/data/uploads"

# 2. Secrets, generated here
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
! grep -q '=CHANGE_ME$' .env.dev || { echo "ERROR: .env.dev still has CHANGE_ME"; exit 1; }
for k in $(sed -n 's/=CHANGE_ME$//p' .env.dev.example); do
  grep -q "^$k=." .env.dev || { echo "ERROR: .env.dev lacks $k (example gained a key: add it by hand on the box)"; exit 1; }
done

# 3. Caddy
CF=/opt/caddy/Caddyfile
reload() { docker exec caddy caddy reload --config /etc/caddy/Caddyfile --adapter caddyfile; }
msgview_ok() { # msgview's real page from the origin on this box, not a CDN copy
  local body
  body=$(curl -fsS --max-time 20 --resolve msgview.app:443:127.0.0.1 https://msgview.app/) || return 1
  grep -qi '<title>[^<]*msgview' <<<"$body"
}
if grep -q '^underground-dev\.65-108-87-247\.sslip\.io {' "$CF"; then
  echo "→ Caddy blocks already present, kept"
else
  msgview_ok || { echo "ERROR: msgview origin check fails BEFORE any change; not touching Caddy"; exit 1; }
  BK=/opt/backups/Caddyfile.pre-underground-dev-$(date +%F-%H%M%S)
  cp -a "$CF" "$BK"
  echo "$BK" > "$ROOT/caddy-backup.path"
  { echo; cat caddy-underground-dev.snippet; } >> "$CF"
  if docker exec caddy caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile >/dev/null 2>&1 \
     && reload && sleep 3 && msgview_ok; then
    echo "→ Caddy blocks added and reloaded; msgview origin serves its page (backup $BK)"
    echo "CADDY_CHANGED"
  else
    cat "$BK" > "$CF"   # in place: same inode as the container's bind mount
    reload || true
    echo "ERROR: Caddy change failed validation or broke msgview; restored $BK"; exit 1
  fi
fi
REMOTE
# Only a run that changed Caddy runs msgview's browser smoke and may undo: undoing an older change would also
# discard any later edit to the shared file.
grep -qx CADDY_CHANGED "$LOG" || exit 0

# msgview's own browser smoke, from outside the box. Red undoes the Caddy change.
TAG=ug-bootstrap-$(date +%s)
gh workflow run prod-smoke.yml -R SerSSX/msgview -f base_url=https://msgview.app -f run_tag="$TAG" >/dev/null
for _ in $(seq 1 30); do
  RID=$(gh run list -R SerSSX/msgview --workflow prod-smoke.yml --event workflow_dispatch -L 10 \
          --json databaseId,displayTitle -q ".[] | select(.displayTitle | contains(\"$TAG\")) | .databaseId" | head -1)
  [ -n "$RID" ] && break; sleep 5
done
[ -n "${RID:-}" ] || { echo "ERROR: msgview smoke run not found"; exit 1; }
gh run watch "$RID" -R SerSSX/msgview --exit-status >/dev/null 2>&1 && { echo "→ msgview browser smoke green (run $RID)"; exit 0; }
echo "msgview browser smoke RED (run $RID) — undoing the Caddy change"
ssh_box 'BK=$(cat /opt/underground-dev/caddy-backup.path 2>/dev/null) && [ -f "$BK" ] && cat "$BK" > /opt/caddy/Caddyfile \
  && docker exec caddy caddy reload --config /etc/caddy/Caddyfile --adapter caddyfile && echo "restored $BK"'
exit 1

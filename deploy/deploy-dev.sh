#!/usr/bin/env bash
# Deploy one CI-green commit to the Underground DEV stand. Idempotent; safe to re-run.
#
#   bash deploy/deploy-dev.sh            # the newest commit of the DEV branch whose CI is green
#   bash deploy/deploy-dev.sh <commit>   # a specific commit — also the rollback (see releases.jsonl)
#
# What CI tested is what ships (rules §6): this script builds nothing. It refuses a commit unless the CI push
# run on the DEV branch for that exact commit succeeded with every gate job and the image job green, takes the
# arm64 images CI built for that commit (or the copy already on the box, so a rollback outlives the 3-day
# artifact retention), and the compose/smoke files from git at that commit. A red smoke puts the previous
# release back by itself and says whether that worked. Every outcome is appended to releases.jsonl on the box.
set -euo pipefail
cd "$(dirname "$0")/.."

REPO=SerSSX/underground
BRANCH=claude/underground-mvp
HOST=${UG_DEV_HOST:-root@65.108.87.247}
KEY=${UG_DEV_SSH_KEY:-$HOME/.ssh/hetzner}
ROOT=/opt/underground-dev
MIN_FREE_MB=3000   # refuse below this much free disk: the box also serves LIVE products
ssh_box() { ssh -i "$KEY" -o BatchMode=yes -o ConnectTimeout=15 "$HOST" "$@"; }

# Echo "<run id>" if <commit> has a completed CI push run on the DEV branch whose required jobs all succeeded.
green_run() {
  local id
  id=$(gh run list -R "$REPO" --workflow ci.yml --branch "$BRANCH" --commit "$1" --event push \
         --status success --json databaseId -q '.[0].databaseId // empty')
  [ -n "$id" ] || return 1
  gh run view "$id" -R "$REPO" --json jobs -q '
    [.jobs[] | select(.name=="api" or .name=="web (web)" or .name=="web (admin)" or .name=="images")
             | select(.conclusion=="success")] | length' | grep -qx 4 || return 1
  echo "$id"
}

git fetch -q origin "$BRANCH"
if [ -n "${1:-}" ]; then
  SHA=$(git rev-parse --verify "$1^{commit}")
  git merge-base --is-ancestor "$SHA" "origin/$BRANCH" || { echo "REFUSED: $SHA is not on origin/$BRANCH"; exit 1; }
  RUN_ID=$(green_run "$SHA") || { echo "REFUSED: $SHA has no green CI push run on $BRANCH (gates + images)"; exit 1; }
else
  # The branch tip is often a docs-only commit with no CI run: take the newest commit that has a green one.
  SHA=""
  for c in $(git rev-list -n 30 "origin/$BRANCH"); do
    if RUN_ID=$(green_run "$c"); then SHA=$c; break; fi
  done
  [ -n "$SHA" ] || { echo "REFUSED: none of the last 30 commits on $BRANCH has a green CI run"; exit 1; }
fi
echo "→ commit $SHA (CI run $RUN_ID green)"

FREE=$(ssh_box "df -Pm / | awk 'NR==2{print \$4}'")
[ "$FREE" -ge "$MIN_FREE_MB" ] || { echo "REFUSED: only ${FREE} MB free on the box (need $MIN_FREE_MB)"; exit 1; }

TMP=$(mktemp -d); trap 'rm -rf "$TMP"' EXIT
for f in docker-compose.dev.yml smoke-dev.sh; do git show "$SHA:deploy/$f" > "$TMP/$f"; done
ssh_box "mkdir -p $ROOT/incoming/$SHA"
if ssh_box "for i in api web admin; do docker image inspect underground-\$i:$SHA >/dev/null 2>&1 || exit 1; done"; then
  echo "→ images for $SHA already on the box"
else
  # The images CI built for this commit, checked against the checksums CI wrote.
  gh run download "$RUN_ID" -R "$REPO" -n images -D "$TMP"
  (cd "$TMP" && shasum -a 256 -c SHA256SUMS >/dev/null) || { echo "REFUSED: image checksum mismatch"; exit 1; }
fi
scp -q -i "$KEY" -o BatchMode=yes "$TMP"/* "$HOST:$ROOT/incoming/$SHA/"

echo "→ load, switch, smoke"
ssh_box "bash -s" -- "$SHA" <<'REMOTE'
set -euo pipefail
SHA=$1; ROOT=/opt/underground-dev; IN=$ROOT/incoming/$SHA
cd "$ROOT"
mountpoint -q "$ROOT/data" || { echo "REFUSED: $ROOT/data (the 4 GB data fs) is not mounted"; exit 1; }
for f in "$IN"/underground-*.tar.zst; do
  [ -e "$f" ] || continue
  zstd -dc "$f" | docker load -q >/dev/null
  rm -f "$f"          # loaded images live in docker; the tarball is redundant disk use
done
for i in api web admin; do docker image inspect "underground-$i:$SHA" >/dev/null; done
touch "$IN"

PREV=$(sed -n 's/^UG_TAG=//p' release.env 2>/dev/null || true)
compose() { docker compose --env-file .env.dev --env-file release.env -f docker-compose.dev.yml "$@"; }
switch_to() { # tag
  cp "$ROOT/incoming/$1/docker-compose.dev.yml" docker-compose.dev.yml
  echo "UG_TAG=$1" > release.env
  compose up -d --wait --wait-timeout 180
}
record() { # commit outcome
  printf '{"time":"%s","env":"dev","commit":"%s","previous":"%s","outcome":"%s","rollback":"%s"}\n' \
    "$(date -u +%FT%TZ)" "$1" "${PREV:-}" "$2" \
    "$([ -n "${PREV:-}" ] && echo "bash deploy/deploy-dev.sh $PREV" || echo "none: first release")" \
    >> "$ROOT/releases.jsonl"
}

if switch_to "$SHA" && bash "$IN/smoke-dev.sh" "$SHA"; then
  record "$SHA" green
  # Keep the images of the current release, the previous one, and the last three green releases.
  KEEP=$( { echo "$SHA"; echo "${PREV:-}"; grep '"outcome":"green"' releases.jsonl | tail -3 \
            | sed -n 's/.*"commit":"\([0-9a-f]*\)".*/\1/p'; } | sort -u)
  for i in api web admin; do
    for t in $(docker image ls "underground-$i" --format '{{.Tag}}'); do
      grep -qx "$t" <<<"$KEEP" || docker image rm "underground-$i:$t" >/dev/null 2>&1 || true
    done
  done
  for d in "$ROOT"/incoming/*/; do
    grep -qx "$(basename "$d")" <<<"$KEEP" || rm -rf "$d"
  done
  echo "DEPLOYED $SHA"
  exit 0
fi

record "$SHA" red
if [ -z "${PREV:-}" ] || [ "$PREV" = "$SHA" ]; then
  echo "SMOKE RED — no previous release to restore; DEV stays on $SHA"
  exit 1
fi
echo "SMOKE RED — restoring the previous release $PREV"
if switch_to "$PREV" && bash "$ROOT/incoming/$PREV/smoke-dev.sh" "$PREV"; then
  record "$PREV" restored-green; echo "RESTORED $PREV (smoke green)"
else
  record "$PREV" restore-failed; echo "RESTORE FAILED — DEV is down or broken; see docker logs underground-dev-api"
fi
exit 1
REMOTE
# The Android APK CI built for this commit (only when android/ changed): publish it for the /app page.
# Served by the API's /uploads route from the stand's data fs; the manifest makes the site show the banner.
APK_DIR="$TMP/apk"
if gh run download "$RUN_ID" -R "$REPO" -n android-apk -D "$APK_DIR" >/dev/null 2>&1 && [ -s "$APK_DIR/app-debug.apk" ]; then
  SHORT=${SHA:0:7}
  SUM=$(shasum -a 256 "$APK_DIR/app-debug.apk" | cut -d' ' -f1)
  SIZE=$(wc -c < "$APK_DIR/app-debug.apk" | tr -d ' ')
  printf '{"version":"0.1.0-dev.%s","url":"/uploads/app/underground-%s.apk","size_bytes":%s,"sha256":"%s","min_android":"7.0","released_at":"%s"}\n' \
    "$SHORT" "$SHORT" "$SIZE" "$SUM" "$(date -u +%FT%TZ)" > "$APK_DIR/latest.json"
  ssh_box "mkdir -p $ROOT/data/uploads/app && chown 10001:10001 $ROOT/data/uploads/app"
  scp -q -i "$KEY" -o BatchMode=yes "$APK_DIR/app-debug.apk" "$HOST:$ROOT/data/uploads/app/underground-$SHORT.apk"
  scp -q -i "$KEY" -o BatchMode=yes "$APK_DIR/latest.json" "$HOST:$ROOT/data/uploads/app/latest.json"
  ssh_box "cd $ROOT/data/uploads/app && chown 10001:10001 * && ls -t underground-*.apk | tail -n +4 | xargs -r rm -f"
  echo "→ Android APK published: /uploads/app/underground-$SHORT.apk"
else
  echo "→ no Android APK in this CI run (android/ unchanged); the previous one stays"
fi
echo "→ https://underground-dev.65-108-87-247.sslip.io"

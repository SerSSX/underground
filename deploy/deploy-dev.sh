#!/usr/bin/env bash
# Deploy one CI-green commit to the Underground DEV stand. Idempotent; safe to re-run.
#
#   bash deploy/deploy-dev.sh            # newest commit of the DEV branch
#   bash deploy/deploy-dev.sh <commit>   # a specific commit (also the rollback: the previous commit)
#
# What CI tested is what ships (rules §6): this script builds nothing. It refuses a commit whose CI run is not
# green, takes the arm64 images CI built for that exact commit, and the compose/smoke files from git at that
# commit. A red smoke puts the previous release back by itself. Every run appends a record on the box.
set -euo pipefail
cd "$(dirname "$0")/.."

REPO=SerSSX/underground
BRANCH=claude/underground-mvp
HOST=${UG_DEV_HOST:-root@65.108.87.247}
KEY=${UG_DEV_SSH_KEY:-$HOME/.ssh/hetzner}
ROOT=/opt/underground-dev
ssh_box() { ssh -i "$KEY" -o BatchMode=yes -o ConnectTimeout=15 "$HOST" "$@"; }

git fetch -q origin "$BRANCH"
SHA=$(git rev-parse --verify "${1:-origin/$BRANCH}^{commit}")
git merge-base --is-ancestor "$SHA" "origin/$BRANCH" \
  || { echo "REFUSED: $SHA is not on origin/$BRANCH"; exit 1; }
echo "→ commit $SHA"

# 1. The deploy step refuses on anything but green CI for this exact commit (rules §6: the host cannot).
RUN=$(gh run list -R "$REPO" --workflow ci.yml --commit "$SHA" --event push \
  --json databaseId,status,conclusion -q '.[0] | "\(.databaseId) \(.status) \(.conclusion)"')
read -r RUN_ID RUN_STATUS RUN_CONCL <<<"${RUN:-none none none}"
if [ "$RUN_STATUS" != completed ] || [ "$RUN_CONCL" != success ]; then
  echo "REFUSED: CI for $SHA is '${RUN_STATUS}/${RUN_CONCL}' (run ${RUN_ID}). Deploy only a green commit."
  echo "  (A docs-only commit has no CI run: deploy the newest commit that has one.)"
  exit 1
fi
echo "→ CI run $RUN_ID is green"

# 2. The images CI built for this commit, checked against the checksums CI wrote.
TMP=$(mktemp -d); trap 'rm -rf "$TMP"' EXIT
gh run download "$RUN_ID" -R "$REPO" -n images -D "$TMP"
(cd "$TMP" && shasum -a 256 -c SHA256SUMS >/dev/null) || { echo "REFUSED: image checksum mismatch"; exit 1; }
for f in docker-compose.dev.yml smoke-dev.sh; do git show "$SHA:deploy/$f" > "$TMP/$f"; done
echo "$SHA" > "$TMP/SHA"

echo "→ copy to the box"
ssh_box "mkdir -p $ROOT/incoming/$SHA"
scp -q -i "$KEY" -o BatchMode=yes "$TMP"/* "$HOST:$ROOT/incoming/$SHA/"

echo "→ load, switch, smoke"
ssh_box "bash -s" -- "$SHA" <<'REMOTE'
set -euo pipefail
SHA=$1; ROOT=/opt/underground-dev; IN=$ROOT/incoming/$SHA
cd "$ROOT"
for f in "$IN"/underground-*.tar.zst; do zstd -dc "$f" | docker load -q >/dev/null; done
for i in api web admin; do docker image inspect "underground-$i:$SHA" >/dev/null; done

PREV=$(sed -n 's/^UG_TAG=//p' release.env 2>/dev/null || true)
up() { # tag
  cp "$IN/docker-compose.dev.yml" docker-compose.dev.yml
  echo "UG_TAG=$1" > release.env
  docker compose --env-file .env.dev --env-file release.env -f docker-compose.dev.yml up -d --wait --wait-timeout 180
}
record() { # result
  printf '{"time":"%s","env":"dev","commit":"%s","previous":"%s","smoke":"%s","rollback":"bash deploy/deploy-dev.sh %s"}\n' \
    "$(date -u +%FT%TZ)" "$SHA" "${PREV:-none}" "$1" "${PREV:-<none>}" >> "$ROOT/releases.jsonl"
}

if up "$SHA" && bash "$IN/smoke-dev.sh" "$SHA"; then
  record green
  # Keep the current and two previous releases' images; drop older ones.
  for i in api web admin; do
    docker image ls "underground-$i" --format '{{.CreatedAt}}\t{{.Tag}}' | sort -r | awk 'NR>3{print $NF}' \
      | while read -r t; do docker image rm -f "underground-$i:$t" >/dev/null || true; done
  done
  ls -dt "$ROOT"/incoming/*/ | tail -n +4 | xargs -r rm -rf
  echo "DEPLOYED $SHA"
else
  record red
  if [ -n "${PREV:-}" ] && [ "$PREV" != "$SHA" ]; then
    echo "SMOKE RED — restoring the previous release $PREV"
    cp "$ROOT/incoming/$PREV/docker-compose.dev.yml" docker-compose.dev.yml 2>/dev/null || true
    echo "UG_TAG=$PREV" > release.env
    docker compose --env-file .env.dev --env-file release.env -f docker-compose.dev.yml up -d --wait --wait-timeout 180 \
      && bash "$ROOT/incoming/$PREV/smoke-dev.sh" "$PREV" && echo "restored $PREV"
  fi
  exit 1
fi
REMOTE
echo "→ https://underground-dev.65-108-87-247.sslip.io"

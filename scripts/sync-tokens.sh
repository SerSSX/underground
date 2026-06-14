#!/usr/bin/env bash
# Propagate the single source of truth (shared/design-tokens.css) into each Next app as a
# generated, do-not-edit copy that globals.css @imports. Run after editing the shared file.
set -euo pipefail
cd "$(dirname "$0")/.."

SRC="shared/design-tokens.css"
HEADER="/* GENERATED from $SRC — DO NOT EDIT. Run scripts/sync-tokens.sh after editing the source. */"

for dest in web/app/tokens.css admin/app/tokens.css; do
  { echo "$HEADER"; cat "$SRC"; } > "$dest"
  echo "synced → $dest"
done

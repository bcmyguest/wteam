#!/bin/sh
# Prepare the repository immediately before the historical bulk-dispatch change.
# Usage: sh prepare.sh <output-directory> [wteam]
set -eu

BASE=ba759a4
OUT=$1
MODE=${2:-}
ROOT=$(cd "$(dirname "$0")/../.." && pwd)

rm -rf "$OUT"
mkdir -p "$OUT"
git -C "$ROOT" archive "$BASE" | tar -x -C "$OUT"
cd "$OUT"
git init -q -b main
git config user.email bench@example.com
git config user.name bench
# Inherit neither signing nor a template that blocks unsigned agent commits.
git config commit.gpgsign false
git config tag.gpgsign false
git add -A
git commit -qm "chore: brownfield benchmark base"
if [ "$MODE" = wteam ]; then
  wteam --slug bench init >/dev/null
fi
echo "$OUT: brownfield base $BASE ready"

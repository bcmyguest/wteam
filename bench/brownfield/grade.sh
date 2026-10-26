#!/bin/sh
# Grade a run against the complete suite at the historical target commit.
# Usage: sh grade.sh <repository-directory>
set -eu

TARGET=e8d709d
BASE=ba759a4
REPO=$1
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

tar --exclude=.git --exclude=.wteam --exclude=target -C "$REPO" -cf - . | tar -x -C "$TMP"
rm -rf "$TMP/tests"
git -C "$ROOT" archive "$TARGET" tests | tar -x -C "$TMP"
python3 "$HERE/grader/apply_adaptations.py" "$TMP/tests"
cd "$TMP"
cargo fmt -- tests/roles.rs tests/watcher.rs
echo "grader_base_commit $BASE"
echo "grader_tests_commit $TARGET"
if cargo fmt --check; then
  echo "fmt_pass 1"
else
  echo "fmt_pass 0"
  exit 1
fi
cargo test --no-fail-fast

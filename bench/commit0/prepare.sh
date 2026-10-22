#!/bin/sh
# Builds a "rebuild this library" repo from a Commit0 task (https://github.com/commit-0):
# the library's function bodies are stubbed, its spec is unpacked, and its tests are REMOVED
# (graded later from pristine copies). History is re-initialised so agents cannot recover
# the original implementation from git.
#   sh prepare.sh <lib> <out dir> [wteam]      libs: tinydb voluptuous bitstring marshmallow imapclient (see LIBS below)
set -eu
LIB="$1"; OUT="$2"; MODE="${3:-}"
CACHE="${BENCH_CACHE:-/tmp/commit0-cache}"
# incidentkit: our own task (bench/incidentkit/SPEC.md); its tests live outside every repo.
if [ "$LIB" = incidentkit ]; then
  rm -rf "$OUT" && mkdir -p "$OUT/incidentkit"
  cp "$(cd "$(dirname "$0")/.." && pwd)/incidentkit/SPEC.md" "$OUT/SPEC.md"
  : > "$OUT/incidentkit/__init__.py"
  printf '.pytest_cache/\n__pycache__/\nscratch/\n' > "$OUT/.gitignore"
  cd "$OUT"
  git init -q -b main && git config user.email bench@example.com && git config user.name bench
  git add -A && git -c commit.gpgsign=false commit -qm "chore: incidentkit spec and empty package"
  if [ "$MODE" = wteam ]; then wteam --slug bench init >/dev/null; fi
  echo "$OUT: incidentkit ready"; exit 0
fi
case "$LIB" in
  tinydb) TESTS=tests ;;
  voluptuous) TESTS=voluptuous/tests ;;
  bitstring|marshmallow|imapclient) TESTS=tests ;;
  *) echo "unknown lib $LIB" >&2; exit 2 ;;
esac
mkdir -p "$CACHE"
[ -d "$CACHE/$LIB" ] || git clone -q --depth 1 -b commit0_combined "https://github.com/commit-0/$LIB" "$CACHE/$LIB"
rm -rf "$OUT" && mkdir -p "$OUT"
(cd "$CACHE/$LIB" && tar --exclude=.git -cf - .) | (cd "$OUT" && tar -xf -)
cd "$OUT"
rm -rf "$TESTS" .github
if [ -f spec.pdf.bz2 ]; then bunzip2 spec.pdf.bz2; fi
printf '.pytest_cache/\n__pycache__/\nscratch/\n' >> .gitignore
git init -q -b main && git config user.email bench@example.com && git config user.name bench
git add -A
git -c commit.gpgsign=false commit -qm "chore: stubbed $LIB (commit0) with tests removed"
if [ "$MODE" = wteam ]; then wteam --slug bench init >/dev/null; fi
echo "$OUT: $LIB ready (tests in $TESTS removed)"

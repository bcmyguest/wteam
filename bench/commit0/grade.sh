#!/bin/sh
# Grades a rebuilt library with its PRISTINE test suite (from the Commit0 branch).
#   sh grade.sh <lib> <repo dir>   → "RESULT passed/total" (total = tests the reference passes)
set -u
LIB="$1"; REPO="$2"
CACHE="${BENCH_CACHE:-/tmp/commit0-cache}"
case "$LIB" in
  tinydb) TESTS=tests; TOTAL=201; ARGS="" ;;
  voluptuous) TESTS=voluptuous/tests; TOTAL=149; ARGS="-o python_files=tests.py --doctest-glob=*.md" ;;
  *) echo "unknown lib $LIB" >&2; exit 2 ;;
esac
tmp=$(mktemp -d)
(cd "$REPO" && tar --exclude=.git --exclude=.wteam --exclude=scratch -cf - .) | (cd "$tmp" && tar -xf -)
rm -rf "$tmp/$TESTS" && mkdir -p "$tmp/$TESTS" && cp -R "$CACHE/$LIB/$TESTS/." "$tmp/$TESTS/"
out=$(cd "$tmp" && timeout 600 python3 -m pytest -q -p no:cacheprovider -c /dev/null --rootdir . \
  --continue-on-collection-errors $ARGS "$TESTS" 2>&1)
passed=$(echo "$out" | grep -oE '[0-9]+ passed' | tail -1 | grep -oE '[0-9]+'); passed=${passed:-0}
echo "$out" | tail -3
rm -rf "$tmp"
echo "RESULT $passed/$TOTAL"

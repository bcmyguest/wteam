#!/bin/sh
# Grades a rebuilt library with its PRISTINE test suite (from the Commit0 branch).
#   sh grade.sh <lib> <repo dir>   → "RESULT passed/total" (total = tests the reference passes)
#   Libs with third-party deps run in a venv built by `sh venv.sh <lib>` (/tmp/commit0-venvs/<lib>).
set -u
LIB="$1"; REPO="$2"
CACHE="${BENCH_CACHE:-/tmp/commit0-cache}"
PY=python3; PP=""   # PY: interpreter, PP: PYTHONPATH (src layouts; the repo copy is not installed)
case "$LIB" in
  tinydb) TESTS=tests; TOTAL=201; ARGS="" ;;
  voluptuous) TESTS=voluptuous/tests; TOTAL=149; ARGS="-o python_files=tests.py --doctest-glob=*.md" ;;
  bitstring) TESTS=tests; TOTAL=834; ARGS="" ;;
  marshmallow) TESTS=tests; TOTAL=1228; ARGS=""; PP=src ;;   # test_version_attributes needs installed metadata
  imapclient) TESTS=tests; TOTAL=267; ARGS="" ;;
  *) echo "unknown lib $LIB" >&2; exit 2 ;;
esac
case "$LIB" in bitstring|marshmallow|imapclient) PY="${BENCH_VENVS:-/tmp/commit0-venvs}/$LIB/bin/python" ;; esac
tmp=$(mktemp -d)
(cd "$REPO" && tar --exclude=.git --exclude=.wteam --exclude=scratch -cf - .) | (cd "$tmp" && tar -xf -)
rm -rf "$tmp/$TESTS" && mkdir -p "$tmp/$TESTS" && cp -R "$CACHE/$LIB/$TESTS/." "$tmp/$TESTS/"
out=$(cd "$tmp" && PYTHONPATH="$PP" timeout 600 "$PY" -m pytest -q -p no:cacheprovider -c /dev/null --rootdir . \
  --continue-on-collection-errors $ARGS "$TESTS" 2>&1)
passed=$(echo "$out" | grep -oE '[0-9]+ passed' | tail -1 | grep -oE '[0-9]+'); passed=${passed:-0}
echo "$out" | tail -3
rm -rf "$tmp"
echo "RESULT $passed/$TOTAL"

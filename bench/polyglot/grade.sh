#!/bin/sh
# Grades a benchmark repo against PRISTINE test files, so agent edits to tests never count.
#   sh grade.sh <polyglot-benchmark checkout> <repo dir>
# Prints PASS/FAIL per exercise and a final "RESULT passed/total".
set -u
SRC="$1/python/exercises/practice"; REPO="$2"
pass=0; total=0; tmp=$(mktemp -d)
for d in "$SRC"/*/; do
  [ -n "${BENCH_LIMIT:-}" ] && [ "${seen:=0}" -ge "$BENCH_LIMIT" ] && break; seen=$((${seen:-0}+1))
  ex=$(basename "$d"); mod=$(echo "$ex" | tr - _); total=$((total+1))
  mkdir -p "$tmp/$ex"
  cp "$REPO/$ex/$mod.py" "$tmp/$ex/" 2>/dev/null
  for f in "$d"*.py; do   # pristine tests and helpers, never the agent's copies
    [ "$(basename "$f")" = "$mod.py" ] || cp "$f" "$tmp/$ex/"
  done
  if (cd "$tmp/$ex" && timeout 60 python3 -m pytest -q -p no:cacheprovider "${mod}_test.py" >/dev/null 2>&1); then
    pass=$((pass+1)); echo "PASS $ex"
  else
    echo "FAIL $ex"
  fi
done
rm -rf "$tmp"
echo "RESULT $pass/$total"

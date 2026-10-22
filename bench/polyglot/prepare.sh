#!/bin/sh
# Builds a benchmark repo from Aider's polyglot benchmark (Python track):
# one directory per exercise with the stub, its unit tests and instructions.
#   sh prepare.sh <polyglot-benchmark checkout> <out dir> [wteam]   (BENCH_LIMIT=N: first N exercises)
# With "wteam", also writes one ticket per exercise under .wteam/bench/.
set -eu
SRC="$1/python/exercises/practice"; OUT="$2"; MODE="${3:-}"
rm -rf "$OUT" && mkdir -p "$OUT" && cd "$OUT"
git init -q -b main && git config user.email bench@example.com && git config user.name bench
printf '.pytest_cache/\n__pycache__/\n' > .gitignore
n=0
for d in "$SRC"/*/; do
  [ -n "${BENCH_LIMIT:-}" ] && [ "${seen:=0}" -ge "$BENCH_LIMIT" ] && break; seen=$((${seen:-0}+1))
  ex=$(basename "$d"); mod=$(echo "$ex" | tr - _)
  mkdir -p "$ex"
  cp "$d"*.py "$ex/"   # stub, tests and any helpers (e.g. paasio/test_utils.py)
  cat "$d.docs/instructions.md" > "$ex/INSTRUCTIONS.md"
  if [ -f "$d.docs/instructions.append.md" ]; then cat "$d.docs/instructions.append.md" >> "$ex/INSTRUCTIONS.md"; fi
  n=$((n+1))
done
git add -A
git -c commit.gpgsign=false commit -qm "chore: polyglot python exercises ($n)"
if [ "$MODE" = wteam ]; then
  wteam --slug bench init >/dev/null
  i=0
  for d in */; do
    ex=${d%/}; mod=$(echo "$ex" | tr - _); i=$((i+1)); id=$(printf '%02d' $i)
    printf '# %s: Solve %s\n\n**What to build:** implement `%s/%s.py` so that `%s/%s_test.py` passes. The exercise statement is in `%s/INSTRUCTIONS.md`. Do not edit the test file.\n\n**Blocked by:** None (can start immediately)\n\n**Status:** ready-for-agent\n\n**Files:** %s/%s.py\n\n**Verify:** python3 -m pytest -q %s/%s_test.py\n\n- [ ] every test in `%s/%s_test.py` passes\n' \
      "$id" "$ex" "$ex" "$mod" "$ex" "$mod" "$ex" "$ex" "$mod" "$ex" "$mod" "$ex" "$mod" > ".wteam/bench/tickets/$id-$ex.md"
  done
  printf '# Polyglot benchmark (Python)\n\n## Problem Statement\n\nSolve every exercise: each stub must pass its unit tests.\n' > .wteam/bench/spec.md
fi
echo "$OUT: $n exercises"

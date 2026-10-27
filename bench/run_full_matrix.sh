#!/bin/sh
# Commit0 lib + brownfield, three context modes each (sequential suites).
# Usage: sh run_full_matrix.sh [base-dir]
# Env: BENCH_LIBS (default tinydb), BENCH_CLI, BENCH_MODEL, PAIRS, RUN_TIMEOUT
set -eu

BASE=${1:-/tmp/wteam-full-bench-$(date +%Y%m%d)}
ROOT=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$BASE"
echo "full_matrix_base $BASE" | tee "$BASE/status.txt"
echo "started $(date -Iseconds)" >>"$BASE/status.txt"

echo "=== commit0 runner matrix ===" | tee -a "$BASE/status.txt"
BENCH_LIBS=${BENCH_LIBS:-tinydb} PAIRS=${COMMIT0_PAIRS:-4} sh "$ROOT/runner/run_matrix.sh" "$BASE/commit0" \
  >>"$BASE/commit0-matrix.log" 2>&1 || echo "commit0 matrix exit $?" >>"$BASE/status.txt"

echo "=== brownfield matrix ===" | tee -a "$BASE/status.txt"
PAIRS=${BF_PAIRS:-2} sh "$ROOT/brownfield/run_matrix.sh" "$BASE/brownfield" \
  >>"$BASE/brownfield-matrix.log" 2>&1 || echo "brownfield matrix exit $?" >>"$BASE/status.txt"

echo "finished $(date -Iseconds)" >>"$BASE/status.txt"
echo "=== full matrix summary ==="
cat "$BASE/status.txt"

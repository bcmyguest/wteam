#!/bin/sh
# Smoke + cursor continuous on N tasks (team arm skipped until custom agent exists).
# Usage: sh run_matrix.sh [n_tasks]
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
N=${1:-3}
BASE=${TB_MATRIX_DIR:-/tmp/wteam-tb4-matrix}
mkdir -p "$BASE"
export TB_JOBS_DIR="$BASE/jobs"
{
  echo "matrix_base $BASE"
  echo "dataset ${TB_DATASET:-terminal-bench/terminal-bench}"
  echo "n_tasks $N"
} >"$BASE/status.txt"

echo "=== oracle smoke ===" | tee -a "$BASE/status.txt"
sh "$HERE/run_smoke.sh" 1 >>"$BASE/smoke.log" 2>&1 || echo "smoke exit $?" >>"$BASE/status.txt"

echo "=== cursor continuous ===" | tee -a "$BASE/status.txt"
TB_JOB_PREFIX=tb-matrix sh "$HERE/run_continuous.sh" "$N" >>"$BASE/continuous.log" 2>&1 || \
  echo "continuous exit $?" >>"$BASE/status.txt"

echo "=== done ==="
cat "$BASE/status.txt"
ls -la "$TB_JOBS_DIR" 2>/dev/null | tail -5

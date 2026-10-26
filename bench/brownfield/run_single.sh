#!/bin/sh
# Run one Cursor agent (Composer) session on the brownfield task.
# Usage: sh run_single.sh <output-directory> <log-directory>
set -eu

OUT=$1
LOGS=$2
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/common.sh"
require_cmd agent python3 cargo
MODEL=${WTEAM_BENCH_MODEL:-composer-2.5}
RUN_TIMEOUT=${RUN_TIMEOUT:-1800}

sh "$HERE/prepare.sh" "$OUT"
rm -rf "$LOGS"
mkdir -p "$LOGS"
PROMPT=$(cat "$HERE/task.md")
START=$(date +%s)
timeout "$((RUN_TIMEOUT + 600))" agent -p --output-format stream-json --trust --force \
  --model "$MODEL" --workspace "$OUT" "$PROMPT" \
  >"$LOGS/run.jsonl" 2>"$LOGS/stderr.log"
END=$(date +%s)
echo "wall_seconds $((END - START))" >"$LOGS/metrics.txt"
python3 "$HERE/token_usage.py" "$LOGS" >>"$LOGS/metrics.txt"
finish_metrics "$LOGS" "$OUT" "$HERE"
if sh "$HERE/grade.sh" "$OUT" >"$LOGS/grade.txt" 2>&1; then
  echo "grade_pass 1" >>"$LOGS/metrics.txt"
  tail -n 5 "$LOGS/grade.txt"
else
  echo "grade_pass 0" >>"$LOGS/metrics.txt"
  tail -n 12 "$LOGS/grade.txt"
  exit 1
fi

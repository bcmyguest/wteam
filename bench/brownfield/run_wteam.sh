#!/bin/sh
# Run the current wteam topology with Cursor agent (Composer) on the brownfield task.
# Usage: sh run_wteam.sh <output-directory> <log-directory> [pairs]
set -eu

OUT=$1
LOGS=$2
PAIRS=${3:-2}
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/common.sh"
require_cmd agent python3 cargo
ensure_bench_wteam
WTEAM=$(wteam_bin)
MODEL=${WTEAM_BENCH_MODEL:-composer-2.5}
WORKER_MODEL=${WTEAM_BENCH_WORKER_MODEL:-$MODEL}
REVIEWER_MODEL=${WTEAM_BENCH_REVIEWER_MODEL:-$MODEL}
RUN_TIMEOUT=${RUN_TIMEOUT:-1800}

sh "$HERE/prepare.sh" "$OUT" wteam
rm -rf "$LOGS"
mkdir -p "$LOGS"
mkdir -p "$OUT/.wteam/agents"
for ROLE in planner orchestrator worker reviewer; do
  PROFILE=edit
  ROLE_MODEL=$MODEL
  case "$ROLE" in
    planner) PROFILE=plan ;;
    orchestrator) PROFILE=control ;;
    worker) PROFILE=edit; ROLE_MODEL=$WORKER_MODEL ;;
    reviewer) PROFILE=review; ROLE_MODEL=$REVIEWER_MODEL ;;
  esac
  printf 'name = "%s"\ncli = "cursor"\nmodel = "%s"\nprompt = "%s"\nprofile = "%s"\nextra_args = ["--force"]\n' \
    "$ROLE" "$ROLE_MODEL" "$ROLE" "$PROFILE" >"$OUT/.wteam/agents/$ROLE.toml"
done

GOAL=$(cat "$HERE/task.md")
PLAN_PROMPT="$("$WTEAM" --root "$OUT" --slug bench role planner)

Goal:
$GOAL

Slug: bench
Write the spec and 2-6 implementation tickets under .wteam/bench/. Use the existing code and tests for Context and Verify. Finish only after wteam --slug bench pickup reports no problems."
START=$(date +%s)
timeout "$((RUN_TIMEOUT + 600))" agent -p --output-format stream-json --trust --force \
  --model "$MODEL" --workspace "$OUT" "$PLAN_PROMPT" \
  >"$LOGS/plan.jsonl" 2>"$LOGS/plan.stderr.log"
"$WTEAM" --root "$OUT" --slug bench pickup >"$LOGS/pickup.txt"
timeout "$((RUN_TIMEOUT + 600))" "$WTEAM" --root "$OUT" --slug bench run --pairs "$PAIRS" --dispatch auto --replan --timeout "$RUN_TIMEOUT" --max-turns 200 \
  >"$LOGS/run.txt" 2>&1
END=$(date +%s)
echo "wall_seconds $((END - START))" >"$LOGS/metrics.txt"
python3 "$HERE/token_usage.py" "$LOGS" "$OUT/.wteam/bench/logs" >>"$LOGS/metrics.txt"
finish_metrics "$LOGS" "$OUT" "$HERE"
if sh "$HERE/grade.sh" "$OUT" >"$LOGS/grade.txt" 2>&1; then
  echo "grade_pass 1" >>"$LOGS/metrics.txt"
  tail -n 5 "$LOGS/grade.txt"
else
  echo "grade_pass 0" >>"$LOGS/metrics.txt"
  tail -n 12 "$LOGS/grade.txt"
  exit 1
fi

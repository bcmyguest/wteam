#!/bin/sh
# wteam planner + pairs on the brownfield task.
# Usage: sh run_team.sh <output-directory> <log-directory> [pairs]
set -eu

OUT=$1
LOGS=$2
PAIRS=${3:-2}
HERE=$(cd "$(dirname "$0")" && pwd)
RUNNER=$(cd "$HERE/../runner" && pwd)
. "$HERE/common.sh"
. "$RUNNER/config.sh"
. "$RUNNER/invoke.sh"
. "$RUNNER/wteam_setup.sh"
bench_export_models
bench_require_cli
require_cmd python3 cargo
ensure_bench_wteam
WTEAM=$(wteam_bin)
GOAL=$(brownfield_goal "$HERE")
RUN_TIMEOUT=${RUN_TIMEOUT:-1800}
ISOLATION=${WTEAM_BENCH_ISOLATION:-none}

sh "$HERE/prepare.sh" "$OUT" wteam
export WTEAM_FAKE_DIR="${WTEAM_FAKE_DIR:-$OUT/.wteam-fake}"
rm -rf "$LOGS"
mkdir -p "$LOGS"
bench_setup_wteam_agents "$OUT" ""

plan_prompt=$(brownfield_plan_prompt "$WTEAM" "$OUT" "$GOAL" "$PAIRS")
start=$(date +%s)
case $(bench_cli) in
  claude) plog="$LOGS/plan.json" ;;
  *) plog="$LOGS/plan.jsonl" ;;
esac
bench_invoke "$OUT" "$plog" "$plan_prompt" "$((RUN_TIMEOUT + 600))"
plan_end=$(date +%s)
"$WTEAM" --root "$OUT" --slug bench pickup >"$LOGS/pickup.txt" 2>&1
timeout $((RUN_TIMEOUT + 600)) "$WTEAM" --root "$OUT" --slug bench run --pairs "$PAIRS" \
  --isolation "$ISOLATION" --dispatch auto --replan --timeout "$RUN_TIMEOUT" --max-turns 200 \
  >"$LOGS/run.txt" 2>&1
end=$(date +%s)
{
  echo "wall_seconds $((end - start))"
  echo "plan_seconds $((plan_end - start))"
  echo "bench_mode team"
  echo "bench_context team"
  echo "bench_task brownfield"
  echo "bench_cli $(bench_cli)"
  echo "bench_isolation $ISOLATION"
  echo "bench_review ${BENCH_REVIEW:-autopass}"
  echo "bench_pairs $PAIRS"
} >"$LOGS/metrics.txt"
brownfield_finish_run "$LOGS" "$OUT" "$HERE"

#!/bin/sh
# wteam: planner → tickets → worker/reviewer pairs (same shape as bench/commit0/run_wteam.sh).
# Usage:
#   sh run_team.sh <lib> <work-dir> <pairs> [model]
#   sh run_team.sh <lib> <work-dir> <planner> <worker> <reviewer> <pairs>
# Env: CODEGRAPH, SHAPE=coupled, RUN_TIMEOUT, OVERLAP, BENCH_REVIEW=autopass|on,
#      WTEAM_BENCH_ISOLATION (default worktree), BENCH_CLI, BENCH_*_MODEL
set -eu

RUNNER=$(cd "$(dirname "$0")" && pwd)
. "$RUNNER/config.sh"
. "$RUNNER/invoke.sh"
. "$RUNNER/wteam_setup.sh"
. "$RUNNER/finish.sh"

LIB=$1
DIR=$2
if [ $# -ge 6 ]; then
  BENCH_PLANNER_MODEL=$3 BENCH_WORKER_MODEL=$4 BENCH_REVIEWER_MODEL=$5
  PAIRS=$6
  export BENCH_PLANNER_MODEL BENCH_WORKER_MODEL BENCH_REVIEWER_MODEL
elif [ $# -ge 3 ]; then
  PAIRS=$3
  [ -n "${4:-}" ] && BENCH_MODEL=$4 export BENCH_MODEL
else
  PAIRS=${BENCH_PAIRS:-4}
fi

bench_require_cli
require_cmd python3
ensure_bench_wteam
WTEAM=$(wteam_bin)
ISOLATION=${WTEAM_BENCH_ISOLATION:-worktree}
RUN_TIMEOUT=${RUN_TIMEOUT:-1800}

sh "$COMMIT0/prepare.sh" "$LIB" "$DIR" wteam
LOGS="${DIR%/}.logs"
rm -rf "$LOGS"
mkdir -p "$LOGS"
export WTEAM_FAKE_DIR="${WTEAM_FAKE_DIR:-$DIR/.wteam-fake}"
cd "$DIR"
. "$COMMIT0/goal.sh"

bench_codegraph_setup "$DIR"
bench_setup_wteam_agents "$DIR" "${CG:-}"

plan_prompt=$(bench_plan_prompt "$GOAL" "$PAIRS" "$WTEAM" "$DIR")
start=$(date +%s)
case $(bench_cli) in
  claude) plog="$LOGS/plan.json" ;;
  *) plog="$LOGS/plan.jsonl" ;;
esac
bench_invoke "$DIR" "$plog" "$plan_prompt" "$((RUN_TIMEOUT + 600))"
plan_end=$(date +%s)
"$WTEAM" --root "$DIR" --slug bench pickup >"$LOGS/pickup.txt" 2>&1
timeout $((RUN_TIMEOUT + 600)) "$WTEAM" --root "$DIR" --slug bench run --pairs "$PAIRS" \
  --isolation "$ISOLATION" --dispatch auto --timeout "$RUN_TIMEOUT" --max-turns 400 --replan \
  ${OVERLAP:+--overlap "$OVERLAP"} >"$LOGS/run.txt" 2>&1
end=$(date +%s)

{
  echo "wall_seconds $((end - start))"
  echo "plan_seconds $((plan_end - start))"
  echo "tickets $(ls .wteam/bench/tickets 2>/dev/null | wc -l)"
  echo "bench_context team"
  echo "bench_isolation $ISOLATION"
  echo "bench_review ${BENCH_REVIEW:-autopass}"
  echo "bench_pairs $PAIRS"
} >"$LOGS/metrics.txt"
bench_metrics_header "$LOGS" team "$LIB"
grep -E "^Run stopped" "$LOGS/run.txt" 2>/dev/null >>"$LOGS/metrics.txt" || true
bench_record_metrics "$LOGS" "$DIR"
bench_finish_commit0 "$LIB" "$DIR" "$LOGS"
sh "$COMMIT0/archive.sh" "$DIR"
cat "$LOGS/metrics.txt"

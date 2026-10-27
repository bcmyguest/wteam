#!/bin/sh
# wteam: one ticket per exercise (from prepare.sh), worker/reviewer pairs.
# Usage: sh run_polyglot_team.sh <polyglot-checkout> <work-dir> <pairs> [model]
# Env: HIDDEN=1, BENCH_LIMIT=N, DISPATCH=auto|model, BENCH_CLI, BENCH_REVIEW, RUN_TIMEOUT
set -eu

RUNNER=$(cd "$(dirname "$0")" && pwd)
. "$RUNNER/config.sh"
. "$RUNNER/wteam_setup.sh"
. "$RUNNER/finish.sh"

POLYGLOT_SRC=$1
DIR=$2
PAIRS=$3
export POLYGLOT_SRC
[ -n "${4:-}" ] && BENCH_MODEL=$4 export BENCH_MODEL

bench_require_cli
require_cmd python3
ensure_bench_wteam
WTEAM=$(wteam_bin)
ISOLATION=${WTEAM_BENCH_ISOLATION:-none}
RUN_TIMEOUT=${RUN_TIMEOUT:-1800}

sh "$POLYGLOT/prepare.sh" "$POLYGLOT_SRC" "$DIR" wteam
LOGS="${DIR%/}.logs"
rm -rf "$LOGS"
mkdir -p "$LOGS"
export WTEAM_FAKE_DIR="${WTEAM_FAKE_DIR:-$DIR/.wteam-fake}"
cd "$DIR"

bench_setup_polyglot_agents "$DIR"

start=$(date +%s)
timeout $((RUN_TIMEOUT + 600)) "$WTEAM" --root "$DIR" --slug bench run --pairs "$PAIRS" \
  --isolation "$ISOLATION" --timeout "$RUN_TIMEOUT" --max-turns 300 \
  ${DISPATCH:+--dispatch "$DISPATCH"} >"$LOGS/run.txt" 2>&1
end=$(date +%s)

{
  echo "wall_seconds $((end - start))"
  echo "bench_context team"
  echo "bench_isolation $ISOLATION"
  echo "bench_review ${BENCH_REVIEW:-autopass}"
  echo "bench_pairs $PAIRS"
  echo "tickets $(ls .wteam/bench/tickets 2>/dev/null | wc -l)"
} >"$LOGS/metrics.txt"
bench_metrics_header "$LOGS" team polyglot
grep -E "^Run stopped" "$LOGS/run.txt" 2>/dev/null >>"$LOGS/metrics.txt" || true
bench_record_metrics "$LOGS" "$DIR"
bench_finish_polyglot "$POLYGLOT_SRC" "$DIR" "$LOGS"
cat "$LOGS/metrics.txt"

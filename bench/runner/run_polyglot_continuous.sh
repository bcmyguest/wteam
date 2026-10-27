#!/bin/sh
# One agent session for all polyglot exercises (Aider Python track).
# Usage: sh run_polyglot_continuous.sh <polyglot-checkout> <work-dir> [model]
# Env: HIDDEN=1, BENCH_LIMIT=N, BENCH_CLI, BENCH_MODEL, RUN_TIMEOUT
set -eu

RUNNER=$(cd "$(dirname "$0")" && pwd)
. "$RUNNER/config.sh"
. "$RUNNER/invoke.sh"
. "$RUNNER/finish.sh"

POLYGLOT_SRC=$1
DIR=$2
export POLYGLOT_SRC
[ -n "${3:-}" ] && BENCH_MODEL=$3 export BENCH_MODEL

bench_require_cli
require_cmd python3

sh "$POLYGLOT/prepare.sh" "$POLYGLOT_SRC" "$DIR"
LOGS="${DIR%/}.logs"
rm -rf "$LOGS"
mkdir -p "$LOGS"
cd "$DIR"
. "$POLYGLOT/goal.sh"

start=$(date +%s)
case $(bench_cli) in
  claude) log="$LOGS/result.json" ;;
  *) log="$LOGS/run.jsonl" ;;
esac
bench_invoke "$DIR" "$log" "$POLYGLOT_GOAL" "${RUN_TIMEOUT:-5400}"
end=$(date +%s)

{
  echo "wall_seconds $((end - start))"
  echo "bench_context continuous"
} >"$LOGS/metrics.txt"
bench_metrics_header "$LOGS" continuous polyglot
bench_record_metrics "$LOGS" "$DIR"
bench_finish_polyglot "$POLYGLOT_SRC" "$DIR" "$LOGS"
cat "$LOGS/metrics.txt"

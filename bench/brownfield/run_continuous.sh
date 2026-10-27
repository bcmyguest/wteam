#!/bin/sh
# One session on the brownfield task (shared context).
# Usage: sh run_continuous.sh <output-directory> <log-directory>
set -eu

OUT=$1
LOGS=$2
HERE=$(cd "$(dirname "$0")" && pwd)
RUNNER=$(cd "$HERE/../runner" && pwd)
. "$HERE/common.sh"
. "$RUNNER/config.sh"
. "$RUNNER/invoke.sh"
bench_export_models
bench_require_cli
require_cmd python3 cargo
ensure_bench_wteam

sh "$HERE/prepare.sh" "$OUT"
rm -rf "$LOGS"
mkdir -p "$LOGS"
PROMPT=$(brownfield_goal "$HERE")
start=$(date +%s)
case $(bench_cli) in
  claude) log="$LOGS/result.json" ;;
  *) log="$LOGS/run.jsonl" ;;
esac
bench_invoke "$OUT" "$log" "$PROMPT" "${RUN_TIMEOUT:-1800}"
end=$(date +%s)
{
  echo "wall_seconds $((end - start))"
  echo "bench_mode continuous"
  echo "bench_context continuous"
  echo "bench_task brownfield"
  echo "bench_cli $(bench_cli)"
} >"$LOGS/metrics.txt"
brownfield_finish_run "$LOGS" "$OUT" "$HERE"

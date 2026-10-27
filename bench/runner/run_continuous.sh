#!/bin/sh
# One agent session for the whole task (shared context for the full goal).
# Usage: sh run_continuous.sh <lib> <work-dir> [model]
# Env: PLAN=<dir> — same session still, but prompt includes saved spec+tickets (commit0-style).
set -eu

RUNNER=$(cd "$(dirname "$0")" && pwd)
. "$RUNNER/config.sh"
. "$RUNNER/invoke.sh"
. "$RUNNER/finish.sh"

LIB=$1
DIR=$2
[ -n "${3:-}" ] && BENCH_MODEL=$3 export BENCH_MODEL

bench_require_cli
require_cmd python3
ensure_bench_wteam

sh "$COMMIT0/prepare.sh" "$LIB" "$DIR"
LOGS="${DIR%/}.logs"
rm -rf "$LOGS"
mkdir -p "$LOGS"
cd "$DIR"
. "$COMMIT0/goal.sh"

PROMPT="$GOAL Work until every stub is implemented, then report what you checked."
if [ -n "${PLAN:-}" ]; then
  PROMPT="$PROMPT

Follow this plan: work through the tickets one at a time, each only after its Blocked-by tickets.
$(for f in "$PLAN"/spec.md "$PLAN"/[0-9]*.md; do
  [ -f "$f" ] || continue
  printf '\n----- %s -----\n' "$(basename "$f")"
  cat "$f"
done)"
fi

start=$(date +%s)
case $(bench_cli) in
  claude) log="$LOGS/result.json" ;;
  *) log="$LOGS/run.jsonl" ;;
esac
bench_invoke "$DIR" "$log" "$PROMPT" "${RUN_TIMEOUT:-7200}"
end=$(date +%s)

{
  echo "wall_seconds $((end - start))"
  echo "bench_context continuous"
} >"$LOGS/metrics.txt"
bench_metrics_header "$LOGS" continuous "$LIB"
bench_record_metrics "$LOGS" "$DIR"
bench_finish_commit0 "$LIB" "$DIR" "$LOGS"
sh "$COMMIT0/archive.sh" "$DIR"
cat "$LOGS/metrics.txt"

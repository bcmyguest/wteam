#!/bin/sh
# New agent session per exercise ticket (polyglot tickets from prepare.sh).
# Usage: sh run_polyglot_fresh.sh <polyglot-checkout> <work-dir> [model]
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

sh "$POLYGLOT/prepare.sh" "$POLYGLOT_SRC" "$DIR" wteam
LOGS="${DIR%/}.logs"
rm -rf "$LOGS"
mkdir -p "$LOGS"
cd "$DIR"
. "$POLYGLOT/goal.sh"

start=$(date +%s)
n=0
for f in .wteam/bench/tickets/[0-9]*.md; do
  [ -f "$f" ] || continue
  n=$((n + 1))
  id=$(basename "$f" .md)
  PROMPT="$POLYGLOT_GOAL

Fresh session: you only see this ticket in chat (the repo may already contain work from earlier exercises).

----- $id -----
$(cat "$f")

Implement this ticket completely, then stop."
  case $(bench_cli) in
    claude) tlog="$LOGS/fresh-${id}.json" ;;
    *) tlog="$LOGS/fresh-${id}.jsonl" ;;
  esac
  bench_invoke "$DIR" "$tlog" "$PROMPT" "${RUN_TIMEOUT:-1800}"
done
if [ "$n" -eq 0 ]; then
  echo "no tickets under .wteam/bench/tickets" >&2
  exit 2
fi

end=$(date +%s)
{
  echo "wall_seconds $((end - start))"
  echo "bench_context fresh"
  echo "fresh_ticket_runs $n"
} >"$LOGS/metrics.txt"
bench_metrics_header "$LOGS" fresh polyglot
bench_record_metrics "$LOGS" "$DIR"
bench_finish_polyglot "$POLYGLOT_SRC" "$DIR" "$LOGS"
cat "$LOGS/metrics.txt"

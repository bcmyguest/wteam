#!/bin/sh
# Same agent config, new session per ticket (no --resume / no shared chat context).
# Usage: sh run_fresh.sh <lib> <work-dir> [model]
# Env: PLAN=<dir> — use saved tickets; else run planner once, then one invoke per ticket.
set -eu

RUNNER=$(cd "$(dirname "$0")" && pwd)
. "$RUNNER/config.sh"
. "$RUNNER/invoke.sh"
. "$RUNNER/wteam_setup.sh"
. "$RUNNER/finish.sh"

LIB=$1
DIR=$2
[ -n "${3:-}" ] && BENCH_MODEL=$3 export BENCH_MODEL

bench_require_cli
require_cmd python3
ensure_bench_wteam
WTEAM=$(wteam_bin)
RUN_TIMEOUT=${RUN_TIMEOUT:-7200}

sh "$COMMIT0/prepare.sh" "$LIB" "$DIR" wteam
LOGS="${DIR%/}.logs"
rm -rf "$LOGS"
mkdir -p "$LOGS"
export WTEAM_FAKE_DIR="${WTEAM_FAKE_DIR:-$DIR/.wteam-fake}"
cd "$DIR"
. "$COMMIT0/goal.sh"

start=$(date +%s)
ticket_src=
if [ -n "${PLAN:-}" ]; then
  ticket_src=$PLAN
else
  bench_codegraph_setup "$DIR"
  bench_setup_wteam_agents "$DIR" "${CG:-}"
  plan_prompt=$(bench_plan_prompt "$GOAL" 1 "$WTEAM" "$DIR")
  case $(bench_cli) in
    claude) plog="$LOGS/plan.json" ;;
    *) plog="$LOGS/plan.jsonl" ;;
  esac
  bench_invoke "$DIR" "$plog" "$plan_prompt" "$((RUN_TIMEOUT + 600))"
  "$WTEAM" --root "$DIR" --slug bench pickup >"$LOGS/pickup.txt" 2>&1
  ticket_src="$DIR/.wteam/bench/tickets"
fi

n=0
for f in "$ticket_src"/[0-9]*.md; do
  [ -f "$f" ] || continue
  n=$((n + 1))
  id=$(basename "$f" .md)
  PROMPT="$GOAL

Fresh session: you only see this ticket in chat (the repo may already contain work from earlier tickets).

----- $id -----
$(cat "$f")

Implement this ticket completely, then stop."
  case $(bench_cli) in
    claude) tlog="$LOGS/fresh-${id}.json" ;;
    *) tlog="$LOGS/fresh-${id}.jsonl" ;;
  esac
  bench_invoke "$DIR" "$tlog" "$PROMPT" "$RUN_TIMEOUT"
done
if [ "$n" -eq 0 ]; then
  echo "no tickets under $ticket_src" >&2
  exit 2
fi

end=$(date +%s)
{
  echo "wall_seconds $((end - start))"
  echo "bench_context fresh"
  echo "fresh_ticket_runs $n"
} >"$LOGS/metrics.txt"
bench_metrics_header "$LOGS" fresh "$LIB"
bench_record_metrics "$LOGS" "$DIR"
bench_finish_commit0 "$LIB" "$DIR" "$LOGS"
sh "$COMMIT0/archive.sh" "$DIR"
cat "$LOGS/metrics.txt"

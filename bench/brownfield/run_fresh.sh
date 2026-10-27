#!/bin/sh
# New agent session per ticket on the brownfield task.
# Usage: sh run_fresh.sh <output-directory> <log-directory>
set -eu

OUT=$1
LOGS=$2
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

sh "$HERE/prepare.sh" "$OUT" wteam
export WTEAM_FAKE_DIR="${WTEAM_FAKE_DIR:-$OUT/.wteam-fake}"
rm -rf "$LOGS"
mkdir -p "$LOGS"
bench_setup_wteam_agents "$OUT" ""

plan_prompt=$(brownfield_plan_prompt "$WTEAM" "$OUT" "$GOAL" 2)
start=$(date +%s)
case $(bench_cli) in
  claude) plog="$LOGS/plan.json" ;;
  *) plog="$LOGS/plan.jsonl" ;;
esac
agent_ec=0
bench_invoke "$OUT" "$plog" "$plan_prompt" "$((RUN_TIMEOUT + 600))" || agent_ec=$?
"$WTEAM" --root "$OUT" --slug bench pickup >"$LOGS/pickup.txt" 2>&1 || true

n=0
for f in "$OUT/.wteam/bench/tickets"/[0-9]*.md; do
  [ -f "$f" ] || continue
  n=$((n + 1))
  id=$(basename "$f" .md)
  PROMPT="$GOAL

Fresh session: you only see this ticket in chat.

----- $id -----
$(cat "$f")

Implement this ticket, then stop."
  case $(bench_cli) in
    claude) tlog="$LOGS/fresh-${id}.json" ;;
    *) tlog="$LOGS/fresh-${id}.jsonl" ;;
  esac
  bench_invoke "$OUT" "$tlog" "$PROMPT" "$RUN_TIMEOUT" || agent_ec=$?
done
if [ "$n" -eq 0 ]; then
  echo "no tickets planned" >&2
  agent_ec=2
fi

end=$(date +%s)
{
  echo "wall_seconds $((end - start))"
  echo "bench_mode fresh"
  echo "bench_context fresh"
  echo "fresh_ticket_runs $n"
  echo "bench_task brownfield"
  echo "bench_cli $(bench_cli)"
  echo "agent_exit $agent_ec"
} >"$LOGS/metrics.txt"
brownfield_finish_run "$LOGS" "$OUT" "$HERE" || true
exit "$agent_ec"

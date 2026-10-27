#!/bin/sh
# Brownfield: continuous, fresh, team (sequential). Resumable via RESUME=1.
# Usage: sh run_matrix.sh <base-dir>
set -u

BASE=${1:-/tmp/wteam-bf-matrix}
PAIRS=${PAIRS:-2}
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../.." && pwd)
. "$REPO/bench/spin-up/phase.sh"

mkdir -p "$BASE"
init_matrix_status "$BASE" \
  "bench_task brownfield" \
  "bench_cli ${BENCH_CLI:-cursor}"
bootstrap_phase_done_from_status "$BASE" "brownfield-"

any_fail=0
for mode in continuous fresh team; do
  pname="brownfield-$mode"
  if ! phase_should_run "$BASE" "$pname"; then
    continue
  fi
  out="$BASE/$mode/repo"
  logs="$BASE/$mode/logs"
  phase_running "$BASE" "$pname"
  echo "starting brownfield $mode" | tee -a "$BASE/status.txt"
  case $mode in
    continuous) cmd="sh $HERE/run_continuous.sh $out $logs" ;;
    fresh) cmd="sh $HERE/run_fresh.sh $out $logs" ;;
    team) cmd="sh $HERE/run_team.sh $out $logs $PAIRS" ;;
  esac
  ec=0
  eval "$cmd" >"$BASE/$mode.wrapper.log" 2>&1 || ec=$?
  phase_finish "$BASE" "$pname" "$ec"
  [ "$ec" -eq 0 ] || any_fail=1
  OVERNIGHT=$(dirname "$BASE")
  if [ -f "$OVERNIGHT/README.txt" ]; then
    sh "$REPO/bench/spin-up/summarize_overnight.sh" "$OVERNIGHT" >/dev/null 2>&1 || true
  fi
done

echo "=== brownfield matrix pass done ==="
cat "$BASE/status.txt"
for mode in continuous fresh team; do
  f="$BASE/$mode/logs/metrics.txt"
  [ -f "$f" ] || continue
  echo "--- $mode ---"
  grep -E "^(wall_seconds|bench_|grade_pass|fresh_ticket|agent_exit)" "$f" 2>/dev/null || tail -5 "$f"
done
# Also fail the suite if any prior arm still has .phase_failed and no .phase_done.
for mode in continuous fresh team; do
  if [ -f "$BASE/brownfield-$mode.phase_failed" ] && [ ! -f "$BASE/brownfield-$mode.phase_done" ]; then
    any_fail=1
  fi
done
exit "$any_fail"

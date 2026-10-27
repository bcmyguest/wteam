#!/bin/sh
# Brownfield: continuous, fresh, team (sequential).
# Usage: sh run_matrix.sh <base-dir>
set -eu

BASE=${1:-/tmp/wteam-bf-matrix}
PAIRS=${PAIRS:-2}
HERE=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$BASE"
{
  echo "matrix_base $BASE"
  echo "bench_task brownfield"
  echo "bench_cli ${BENCH_CLI:-cursor}"
} >"$BASE/status.txt"

for mode in continuous fresh team; do
  out="$BASE/$mode/repo"
  logs="$BASE/$mode/logs"
  case $mode in
    continuous) cmd="sh $HERE/run_continuous.sh $out $logs" ;;
    fresh) cmd="sh $HERE/run_fresh.sh $out $logs" ;;
    team) cmd="sh $HERE/run_team.sh $out $logs $PAIRS" ;;
  esac
  echo "starting brownfield $mode"
  if ( eval "$cmd"; ec=$?; echo "$(date -Iseconds) brownfield $mode exit $ec" >>"$BASE/status.txt"; exit $ec ) \
    >"$BASE/$mode.wrapper.log" 2>&1; then
    :
  fi
done

echo "=== brownfield matrix done ==="
cat "$BASE/status.txt"
for mode in continuous fresh team; do
  f="$BASE/$mode/logs/metrics.txt"
  [ -f "$f" ] || continue
  echo "--- $mode ---"
  grep -E "^(wall_seconds|bench_|grade_pass|fresh_ticket)" "$f" 2>/dev/null || tail -5 "$f"
done

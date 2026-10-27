#!/bin/sh
# Same lib, same BENCH_CLI/models, three context modes (sequential).
# Usage: sh run_matrix.sh <base-dir>
# Env: BENCH_LIBS, BENCH_CLI, BENCH_MODEL, PAIRS, RUN_TIMEOUT
set -eu

BASE=${1:-/tmp/wteam-bench-matrix}
LIBS=${BENCH_LIBS:-tinydb}
PAIRS=${PAIRS:-4}
RUNNER=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$BASE"
{
  echo "matrix_base $BASE"
  echo "bench_cli ${BENCH_CLI:-cursor}"
  echo "bench_model ${BENCH_MODEL:-default}"
  echo "libs $LIBS"
} >"$BASE/status.txt"

for lib in $LIBS; do
  for mode in continuous fresh team; do
    out="$BASE/${lib}-${mode}"
    case $mode in
      continuous) cmd="sh $RUNNER/run_continuous.sh $lib $out" ;;
      fresh) cmd="sh $RUNNER/run_fresh.sh $lib $out" ;;
      team) cmd="sh $RUNNER/run_team.sh $lib $out $PAIRS" ;;
    esac
    echo "starting $lib $mode"
    if ( eval "$cmd"; ec=$?; echo "$(date -Iseconds) $lib $mode exit $ec" >>"$BASE/status.txt"; exit $ec ) \
      >"$BASE/${lib}-${mode}.wrapper.log" 2>&1; then
      :
    fi
  done
done

echo "=== matrix done ==="
cat "$BASE/status.txt"
for lib in $LIBS; do
  for mode in continuous fresh team; do
    f="$BASE/${lib}-${mode}.logs/metrics.txt"
    [ -f "$f" ] || continue
    echo "--- $lib $mode ---"
    grep -E "^(wall_seconds|bench_|RESULT|total_tokens|agent_turns|cost_usd|fresh_ticket)" "$f" 2>/dev/null || tail -5 "$f"
  done
done

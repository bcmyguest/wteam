#!/bin/sh
# Polyglot: continuous, fresh, and team for the same BENCH_CLI/model.
# Usage: sh run_matrix.sh <polyglot-checkout> <base-dir> [pairs]
set -eu

SRC=$1
BASE=${2:-/tmp/polyglot-matrix}
PAIRS=${3:-${PAIRS:-4}}
RUNNER=$(cd "$(dirname "$0")" && pwd)/../runner
mkdir -p "$BASE"
{
  echo "matrix_base $BASE"
  echo "bench_polyglot_src $SRC"
  echo "bench_cli ${BENCH_CLI:-cursor}"
  echo "bench_model ${BENCH_MODEL:-default}"
  echo "pairs $PAIRS"
} >"$BASE/status.txt"

for mode in continuous fresh team; do
  out="$BASE/$mode"
  case $mode in
    continuous) cmd="sh $RUNNER/run_polyglot_continuous.sh $SRC $out" ;;
    fresh) cmd="sh $RUNNER/run_polyglot_fresh.sh $SRC $out" ;;
    team) cmd="sh $RUNNER/run_polyglot_team.sh $SRC $out $PAIRS" ;;
  esac
  echo "starting polyglot $mode"
  if ( eval "$cmd"; ec=$?; echo "$(date -Iseconds) polyglot $mode exit $ec" >>"$BASE/status.txt"; exit $ec ) \
    >"$BASE/${mode}.wrapper.log" 2>&1; then
    :
  fi
done

echo "=== polyglot matrix done ==="
cat "$BASE/status.txt"
for mode in continuous fresh team; do
  f="$BASE/${mode}.logs/metrics.txt"
  [ -f "$f" ] || continue
  echo "--- $mode ---"
  grep -E "^(wall_seconds|bench_|RESULT|total_tokens|agent_turns|cost_usd|fresh_ticket)" "$f" 2>/dev/null || tail -5 "$f"
done

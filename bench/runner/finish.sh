# Grade, audit, memorization, metrics.
# shellcheck shell=sh

bench_record_metrics() {
  logs=$1
  repo=$2
  python3 "$RUNNER/metrics.py" "$logs" "$repo" >>"$logs/metrics.txt" 2>/dev/null || \
    python3 "$BF/token_usage.py" "$logs" "$repo/.wteam/bench/logs" >>"$logs/metrics.txt" 2>/dev/null || true
}

bench_finish_commit0() {
  lib=$1
  dir=$2
  logs=$3
  sh "$COMMIT0/grade.sh" "$lib" "$dir" >"$logs/grade.txt"
  tail -1 "$logs/grade.txt" >>"$logs/metrics.txt"
  OWN="$dir" sh "$COMMIT0/audit.sh" "$logs" "$dir/.wteam" >>"$logs/audit.txt" 2>/dev/null || \
    OWN="$dir" sh "$COMMIT0/audit.sh" "$logs" >>"$logs/audit.txt"
  head -1 "$logs/audit.txt" >>"$logs/metrics.txt" 2>/dev/null || true
  sh "$COMMIT0/memorization.sh" "$lib" "$dir" >>"$logs/metrics.txt" 2>/dev/null || true
}

bench_finish_polyglot() {
  src=$1
  dir=$2
  logs=$3
  sh "$POLYGLOT/grade.sh" "$src" "$dir" >"$logs/grade.txt"
  tail -1 "$logs/grade.txt" >>"$logs/metrics.txt"
}

bench_metrics_header() {
  logs=$1
  mode=$2
  lib=$3
  {
    echo "bench_mode $mode"
    echo "bench_cli $(bench_cli)"
    echo "bench_model $(bench_model)"
    echo "bench_lib $lib"
  } >>"$logs/metrics.txt"
  if [ -n "${POLYGLOT_SRC:-}" ]; then
    echo "bench_polyglot_src $POLYGLOT_SRC" >>"$logs/metrics.txt"
  fi
}

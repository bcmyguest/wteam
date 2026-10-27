#!/bin/sh
# harness-tb three-way matrix (sequential): continuous, fresh, team.
# Resumable: RESUME=1 skips modes with .phase_done under matrix base.
# Usage: sh run_matrix.sh [n_tasks]
set -u
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../../.." && pwd)
. "$HERE/common.sh"
. "$REPO/bench/spin-up/phase.sh"

N=${1:-3}
BASE=${TB_HARNESS_MATRIX_DIR:-${TB_HARNESS_DIR:-/tmp/wteam-harness-tb-matrix-$(date +%Y%m%d-%H%M%S)}}
mkdir -p "$BASE"
init_matrix_status "$BASE" \
  "bench_task harness-tb" \
  "dataset ${TB_DATASET:-terminal-bench/terminal-bench}" \
  "n_tasks $N"
bootstrap_phase_done_from_status "$BASE" "harness-tb-"

any_fail=0
for mode in continuous fresh team; do
  pname="harness-tb-$mode"
  if ! phase_should_run "$BASE" "$pname"; then
    continue
  fi
  export TB_HARNESS_DIR="$BASE/$mode"
  export TB_JOB_PREFIX="tb-harness-$mode"
  log="$BASE/$mode.wrapper.log"
  phase_running "$BASE" "$pname"
  echo "starting harness-tb $mode" | tee -a "$BASE/status.txt"
  case $mode in
    continuous) cmd="sh $HERE/run_continuous.sh $N" ;;
    fresh) cmd="sh $HERE/run_fresh.sh $N" ;;
    team) cmd="sh $HERE/run_team.sh $N" ;;
  esac
  ec=0
  eval "$cmd" >>"$log" 2>&1 || ec=$?
  phase_finish "$BASE" "$pname" "$ec"
  [ "$ec" -eq 0 ] || any_fail=1
  parent=$(dirname "$BASE")
  if [ -f "$parent/README.txt" ] || [ -L /tmp/wteam-overnight-latest ]; then
    sh "$REPO/bench/spin-up/summarize_overnight.sh" "$parent" >/dev/null 2>&1 || true
  fi
done

echo "=== harness-tb matrix pass done ==="
cat "$BASE/status.txt"
for mode in continuous fresh team; do
  if [ -f "$BASE/harness-tb-$mode.phase_failed" ] && [ ! -f "$BASE/harness-tb-$mode.phase_done" ]; then
    any_fail=1
  fi
done
exit "$any_fail"

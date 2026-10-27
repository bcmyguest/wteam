#!/bin/sh
# Spin up wteam harness benches (runner + brownfield). CLI is the driver, not the subject.
# Usage: sh bench/spin-up/run.sh [target] [args...]
set -eu
BENCH=$(cd "$(dirname "$0")/.." && pwd)
REPO=$(cd "$BENCH/.." && pwd)
ISOL="$BENCH/run-isolated.sh"
RUN_LOG="${RUN_LOG:-/tmp/wteam-spin-$(date +%Y%m%d-%H%M%S).log}"

usage() {
  cat <<EOF
Usage: sh bench/spin-up/run.sh <target> [args...]

Harness (wteam orchestration — continuous / fresh / team):
  harness                 brownfield three-way matrix (default recommendation)
  harness-brownfield      same
  harness-commit0         Commit0 tinydb three-way matrix
  harness-all             brownfield matrix then commit0 matrix
  harness-tb [n]          Terminal-Bench tasks + harness three-way matrix (default n=3)
  harness-tb-team [n]     TB tasks, wteam team arm only
  harness-brownfield-team | -continuous | -fresh   single brownfield arm

External (NOT the harness — Harbor / terminal tasks):
  external-tb-smoke [n]   oracle smoke (default n=1)
  external-tb-cursor [n]  Harbor agent in Docker (default n=5)

Legacy aliases: brownfield-team, runner-matrix, tb-smoke, tb-cursor, matrix-cursor

Env: BENCH_CLI, BENCH_MODEL, BENCH_REVIEW, PAIRS, WTEAM, MATRIX_BASE, BF_RUN_BASE, RUN_LOG
EOF
}

run_cmd() {
  # shellcheck disable=SC2068
  "$ISOL" "$@" 2>&1 | tee "$RUN_LOG"
  echo "log: $RUN_LOG"
}

ensure_wteam() {
  if [ -n "${WTEAM:-}" ] && [ -x "$WTEAM" ]; then
    return 0
  fi
  if [ -x "$REPO/target/release/wteam" ]; then
    export WTEAM="$REPO/target/release/wteam"
    return 0
  fi
  echo "building wteam release binary..." >&2
  (cd "$REPO" && cargo build --release >/dev/null)
  export WTEAM="$REPO/target/release/wteam"
}

bench_defaults() {
  ensure_wteam
  export BENCH_CLI="${BENCH_CLI:-cursor}"
  export BENCH_MODEL="${BENCH_MODEL:-auto}"
  export BENCH_REVIEW="${BENCH_REVIEW:-autopass}"
}

bf_base() {
  base="${BF_RUN_BASE:-/tmp/wteam-harness-bf-$(date +%Y%m%d-%H%M%S)}"
  mkdir -p "$base"
  printf '%s\n' "$base"
}

bf_arm() {
  mode=$1
  bench_defaults
  base=$(bf_base)
  out="$base/$mode/repo"
  logs="$base/$mode/logs"
  mkdir -p "$(dirname "$out")" "$logs"
  case $mode in
    continuous) run_cmd sh "$BENCH/brownfield/run_continuous.sh" "$out" "$logs" ;;
    fresh) run_cmd sh "$BENCH/brownfield/run_fresh.sh" "$out" "$logs" ;;
    team) run_cmd sh "$BENCH/brownfield/run_team.sh" "$out" "$logs" "${PAIRS:-2}" ;;
    *) echo "unknown bf mode: $mode" >&2; exit 2 ;;
  esac
}

target=${1:-harness}
shift || true

case $target in
  help|-h|--help)
    usage
    exit 0
    ;;
  harness|harness-brownfield)
    bench_defaults
    base="${BF_RUN_BASE:-/tmp/wteam-harness-bf-$(date +%Y%m%d-%H%M%S)}"
    run_cmd sh "$BENCH/brownfield/run_matrix.sh" "$base"
    ;;
  harness-commit0|runner-matrix)
    bench_defaults
    base="${MATRIX_BASE:-/tmp/wteam-harness-commit0-$(date +%Y%m%d-%H%M%S)}"
    run_cmd sh "$BENCH/runner/run_matrix.sh" "$base"
    ;;
  harness-all)
    bench_defaults
    bf="${BF_RUN_BASE:-/tmp/wteam-harness-all-bf-$(date +%Y%m%d-%H%M%S)}"
    co="${MATRIX_BASE:-/tmp/wteam-harness-all-commit0-$(date +%Y%m%d-%H%M%S)}"
    run_cmd sh "$BENCH/brownfield/run_matrix.sh" "$bf"
    export RUN_LOG="${RUN_LOG:-/tmp/wteam-spin-commit0-$(date +%Y%m%d-%H%M%S).log}"
    run_cmd sh "$BENCH/runner/run_matrix.sh" "$co"
    ;;
  harness-tb)
    n=${1:-3}
    bench_defaults
    export TB_HARNESS_DIR="${TB_HARNESS_DIR:-/tmp/wteam-harness-tb-$(date +%Y%m%d-%H%M%S)}"
    export PYTHONPATH="$BENCH/terminal-bench/harness${PYTHONPATH:+:$PYTHONPATH}"
    run_cmd sh "$BENCH/terminal-bench/harness/run_matrix.sh" "$n"
    ;;
  harness-tb-team)
    n=${1:-1}
    bench_defaults
    export TB_HARNESS_DIR="${TB_HARNESS_DIR:-/tmp/wteam-harness-tb-team-$(date +%Y%m%d-%H%M%S)}"
    export PYTHONPATH="$BENCH/terminal-bench/harness${PYTHONPATH:+:$PYTHONPATH}"
    run_cmd sh "$BENCH/terminal-bench/harness/run_team.sh" "$n"
    ;;
  harness-brownfield-team|brownfield-team)
    bf_arm team
    ;;
  harness-brownfield-continuous)
    bf_arm continuous
    ;;
  harness-brownfield-fresh)
    bf_arm fresh
    ;;
  external-tb-smoke|tb-smoke)
    n=${1:-1}
    export TB_JOBS_DIR="${TB_JOBS_DIR:-/tmp/wteam-tb-smoke/jobs}"
    run_cmd sh "$BENCH/terminal-bench/run_smoke.sh" "$n"
    ;;
  external-tb-cursor|tb-cursor)
    n=${1:-5}
    base="${TB_RUN_BASE:-/tmp/wteam-tb-cursor-$(date +%Y%m%d-%H%M%S)}"
    mkdir -p "$base"
    export TB_JOBS_DIR="$base/jobs"
    export TB_AGENT=acp:cursor
    export TB_MODEL="${TB_MODEL:-${BENCH_MODEL:-auto}}"
    export TB_MOUNT_CURSOR_AUTH="${TB_MOUNT_CURSOR_AUTH:-auto}"
    run_cmd sh "$BENCH/terminal-bench/run_continuous.sh" "$n"
    ;;
  matrix-cursor)
    n=${1:-3}
    sh "$0" external-tb-smoke 1
    sh "$0" external-tb-cursor "$n"
    ;;
  *)
    echo "unknown target: $target" >&2
    usage >&2
    exit 2
    ;;
esac

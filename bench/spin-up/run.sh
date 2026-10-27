#!/bin/sh
# One entrypoint for common bench runs (clean env, sensible defaults, log file).
# Usage: sh bench/spin-up/run.sh <target> [args...]
set -eu
BENCH=$(cd "$(dirname "$0")/.." && pwd)
REPO=$(cd "$BENCH/.." && pwd)
ISOL="$BENCH/run-isolated.sh"
RUN_LOG="${RUN_LOG:-/tmp/wteam-spin-$(date +%Y%m%d-%H%M%S).log}"

usage() {
  cat <<EOF
Usage: sh bench/spin-up/run.sh <target> [args...]

  tb-smoke [n]         Harbor oracle smoke (default n=1)
  tb-cursor [n]        Terminal-Bench via Harbor acp:cursor + OAuth bind (default n=5)
  matrix-cursor [n]    tb-smoke then tb-cursor
  brownfield-team      Brownfield wteam team arm (cursor, isolation none)
  runner-matrix        Commit0 tinydb three-way matrix (cursor)
  help

Env: TB_JOBS_DIR, TB_RUN_BASE, TB_MODEL, BENCH_MODEL, WTEAM, RUN_LOG
Log: tee to RUN_LOG (default $RUN_LOG)
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

target=${1:-help}
shift || true

case $target in
  help|-h|--help)
    usage
    exit 0
    ;;
  tb-smoke)
    n=${1:-1}
    export TB_JOBS_DIR="${TB_JOBS_DIR:-/tmp/wteam-tb-smoke/jobs}"
    run_cmd sh "$BENCH/terminal-bench/run_smoke.sh" "$n"
    ;;
  tb-cursor)
    n=${1:-5}
    base="${TB_RUN_BASE:-/tmp/wteam-tb-cursor-$(date +%Y%m%d-%H%M%S)}"
    mkdir -p "$base"
    export TB_JOBS_DIR="${TB_JOBS_DIR:-$base/jobs}"
    export TB_AGENT=acp:cursor
    export TB_MODEL="${TB_MODEL:-${BENCH_MODEL:-cursor/composer-2.5}}"
    export TB_MOUNT_CURSOR_AUTH="${TB_MOUNT_CURSOR_AUTH:-auto}"
    run_cmd sh "$BENCH/terminal-bench/run_continuous.sh" "$n"
    ;;
  matrix-cursor)
    n=${1:-3}
    export RUN_LOG="${RUN_LOG:-/tmp/wteam-spin-matrix-$(date +%Y%m%d-%H%M%S).log}"
    sh "$0" tb-smoke 1
    sh "$0" tb-cursor "$n"
    ;;
  brownfield-team)
    ensure_wteam
    export BENCH_CLI="${BENCH_CLI:-cursor}"
    export BENCH_MODEL="${BENCH_MODEL:-composer-2.5}"
    base="${BF_RUN_BASE:-/tmp/wteam-bf-spin-$(date +%Y%m%d-%H%M%S)}"
    out="$base/out"
    logs="$base/logs"
    mkdir -p "$out" "$logs"
    run_cmd sh "$BENCH/brownfield/run_team.sh" "$out" "$logs"
    ;;
  runner-matrix)
    ensure_wteam
    export BENCH_CLI="${BENCH_CLI:-cursor}"
    export BENCH_MODEL="${BENCH_MODEL:-composer-2.5}"
    base="${MATRIX_BASE:-/tmp/wteam-matrix-spin-$(date +%Y%m%d-%H%M%S)}"
    run_cmd sh "$BENCH/runner/run_matrix.sh" "$base"
    ;;
  *)
    echo "unknown target: $target" >&2
    usage >&2
    exit 2
    ;;
esac

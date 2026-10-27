#!/bin/sh
# Resumable overnight harness (harness-tb + brownfield). Partial runs stay recoverable.
#
# Usage:
#   sh bench/spin-up/overnight.sh
#   OVERNIGHT_BASE=/tmp/wteam-overnight-... RESUME=1 sh bench/spin-up/overnight.sh
#   sh bench/spin-up/resume-overnight.sh
#
# Symlink: /tmp/wteam-overnight-latest → active base
# After each phase: SUMMARY.md rewritten from whatever artifacts exist.
set -u
REPO=$(cd "$(dirname "$0")/../.." && pwd)
. "$REPO/bench/spin-up/phase.sh"

export HOME="${OVERNIGHT_HOME:-/home/b}"
. "$HOME/.cargo/env" 2>/dev/null || true
export RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-stable}"
export PATH="/home/b/.local/bin:/home/b/.cargo/bin:/usr/local/bin:/usr/bin:/bin"
export WTEAM="${WTEAM:-$REPO/target/release/wteam}"
export WTEAM_CURSOR_BIN="${WTEAM_CURSOR_BIN:-/home/b/.local/bin/agent}"
export TB_AGENT_BIN="${TB_AGENT_BIN:-/home/b/.local/bin/agent}"
export TB_CURSOR_AUTH_FILE="${TB_CURSOR_AUTH_FILE:-$HOME/.config/cursor/auth.json}"
export BENCH_CLI="${BENCH_CLI:-cursor}"
export BENCH_MODEL="${BENCH_MODEL:-auto}"
export BENCH_REVIEW="${BENCH_REVIEW:-autopass}"
export PYTHONPATH="$REPO/bench/terminal-bench/harness${PYTHONPATH:+:$PYTHONPATH}"
export RESUME="${RESUME:-0}"

if [ -z "${OVERNIGHT_BASE:-}" ]; then
  OVERNIGHT_BASE="/tmp/wteam-overnight-$(date +%Y%m%d-%H%M%S)"
fi
BASE=$OVERNIGHT_BASE
mkdir -p "$BASE"
ln -sfn "$BASE" /tmp/wteam-overnight-latest

TB_N=${TB_N:-3}
export TB_HARNESS_MATRIX_DIR="$BASE/harness-tb"
BF_BASE="$BASE/brownfield"
# Nested matrices inherit RESUME so completed arms are skipped.
export RESUME

preflight() {
  command -v harbor >/dev/null || { echo "harbor missing"; return 1; }
  command -v agent >/dev/null || { echo "agent missing"; return 1; }
  test -f "$TB_CURSOR_AUTH_FILE" || { echo "missing $TB_CURSOR_AUTH_FILE"; return 1; }
  docker info >/dev/null 2>&1 || { echo "docker not ready"; return 1; }
  test -x "$WTEAM" || (cd "$REPO" && cargo build --release) || return 1
  return 0
}

summarize() {
  sh "$REPO/bench/spin-up/summarize_overnight.sh" "$BASE" >/dev/null 2>&1 || true
}

append_readme() {
  printf '%s\n' "$@" >>"$BASE/README.txt"
}

if ! preflight; then
  echo "preflight failed — fix and: sh $REPO/bench/spin-up/resume-overnight.sh $BASE" >&2
  summarize
  exit 1
fi

if [ ! -f "$BASE/README.txt" ]; then
  {
    echo "started $(date -Iseconds)"
    echo "base $BASE"
    echo "HOME $HOME"
  } >"$BASE/README.txt"
fi
append_readme "pass $(date -Iseconds) resume=$RESUME"

# Top-level phases: whole suite. Nested run_matrix.sh also marks per-arm phases.
run_suite() {
  name=$1
  shift
  if ! phase_should_run "$BASE" "$name"; then
    return 0
  fi
  phase_running "$BASE" "$name"
  append_readme "=== phase $name start $(date -Iseconds) ==="
  echo "=== phase $name ===" | tee -a "$BASE/overnight.log"
  ec=0
  "$@" >>"$BASE/${name}.log" 2>&1 || ec=$?
  phase_finish "$BASE" "$name" "$ec"
  append_readme "=== phase $name end exit=$ec $(date -Iseconds) ==="
  summarize
  # Soft-stop if Cursor usage is exhausted — keep markers so resume retries later.
  if grep -R -l -E "out of usage|Increase limits|usage limit" \
    "$BASE" --include='*.stderr.log' --include='*.log' --include='*.txt' 2>/dev/null \
    | head -1 >/dev/null; then
    echo "usage_limit_detected" >"$BASE/STOP_REASON.txt"
    append_readme "STOP_REASON=usage_limit_detected — resume later with resume-overnight.sh"
    summarize
    return 0
  fi
  return 0
}

run_suite harness-tb sh "$REPO/bench/terminal-bench/harness/run_matrix.sh" "$TB_N"
run_suite brownfield sh "$REPO/bench/brownfield/run_matrix.sh" "$BF_BASE"

append_readme "finished $(date -Iseconds)"
summarize
echo "base=$BASE latest=/tmp/wteam-overnight-latest"
echo "summary=$BASE/SUMMARY.md"
# Exit 0 even with failed phases so nohup/cron treat partial success as recoverable.
exit 0

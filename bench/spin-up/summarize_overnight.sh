#!/bin/sh
# Collect partial or complete overnight results into SUMMARY.md (always exit 0).
# Usage: sh summarize_overnight.sh [base-dir]
set -u
REPO=$(cd "$(dirname "$0")/../.." && pwd)
BASE=${1:-${OVERNIGHT_BASE:-}}
if [ -z "$BASE" ]; then
  if [ -L /tmp/wteam-overnight-latest ]; then
    BASE=$(readlink -f /tmp/wteam-overnight-latest)
  else
    echo "usage: OVERNIGHT_BASE=/path sh summarize_overnight.sh" >&2
    exit 2
  fi
fi
OUT="$BASE/SUMMARY.md"

{
  echo "# Overnight bench summary"
  echo
  echo "Base: \`$BASE\`"
  echo "Generated: $(date -Iseconds)"
  echo
  if [ -f "$BASE/STOP_REASON.txt" ]; then
    echo "**Stop reason:** \`$(cat "$BASE/STOP_REASON.txt")\`"
    echo
  fi
  if [ -f "$BASE/README.txt" ]; then
    echo "## README"
    echo '```'
    cat "$BASE/README.txt"
    echo '```'
    echo
  fi

  echo "## Phase markers"
  echo
  find "$BASE" -maxdepth 3 \( -name '*.phase_done' -o -name '*.phase_failed' -o -name '*.phase_running' \) 2>/dev/null \
    | sort | while read -r f; do
    case $f in
      *.phase_done) echo "- done: \`${f#$BASE/}\` ($(cat "$f"))" ;;
      *.phase_failed)
        ex="${f%.phase_failed}.phase_exit"
        echo "- **failed**: \`${f#$BASE/}\` exit $(cat "$ex" 2>/dev/null || echo '?') at $(cat "$f")"
        ;;
      *.phase_running) echo "- **running**: \`${f#$BASE/}\` since $(cat "$f")" ;;
    esac
  done
  echo

  echo "## status.txt"
  for st in "$BASE/status.txt" "$BASE/harness-tb/status.txt" "$BASE/brownfield/status.txt"; do
    [ -f "$st" ] || continue
    echo
    echo "### \`${st#$BASE/}\`"
    echo '```'
    cat "$st"
    echo '```'
  done
  echo

  echo "## harness-tb (Harbor jobs)"
  HTB="$BASE/harness-tb"
  for mode in continuous fresh team; do
    job=$(find "$HTB/$mode/jobs" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | head -1)
    [ -n "$job" ] || continue
    echo "### $mode"
    if [ -f "$job/result.json" ]; then
      # Prefer compact mean if present
      mean=$(grep -o '"mean"[[:space:]]*:[[:space:]]*[0-9.]*' "$job/result.json" 2>/dev/null | head -1)
      echo "- job: \`$job\` ${mean:+($mean)}"
    fi
    find "$job" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | while read -r trial; do
      r="$trial/result.json"
      [ -f "$r" ] || continue
      reward=$(grep -o '"reward"[[:space:]]*:[[:space:]]*[0-9.]*' "$r" 2>/dev/null | head -1)
      echo "- $(basename "$trial"): ${reward:-reward=?}"
    done
    [ -f "$HTB/$mode.wrapper.log" ] && echo "- log: \`$HTB/$mode.wrapper.log\`"
  done
  echo

  echo "## brownfield metrics"
  for mode in continuous fresh team; do
    f="$BASE/brownfield/$mode/logs/metrics.txt"
    [ -f "$f" ] || continue
    echo "### $mode"
    echo '```'
    cat "$f"
    echo '```'
    g="$BASE/brownfield/$mode/logs/grade.txt"
    [ -f "$g" ] && echo "grade: \`$(head -1 "$g")\`"
    err="$BASE/brownfield/$mode/logs/run.stderr.log"
    [ -f "$err" ] || err="$BASE/brownfield/$mode/logs/plan.stderr.log"
    if [ -f "$err" ] && grep -E -q "out of usage|Increase limits|usage limit" "$err" 2>/dev/null; then
      echo "- **usage limit** in \`$(basename "$err")\`"
    fi
  done
  echo

  echo "## Resume"
  echo '```bash'
  echo "sh $REPO/bench/spin-up/resume-overnight.sh $BASE"
  echo "# or: OVERNIGHT_BASE=$BASE RESUME=1 sh $REPO/bench/spin-up/overnight.sh"
  echo '```'
} >"$OUT"

echo "wrote $OUT"
exit 0

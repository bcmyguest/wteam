#!/bin/sh
# Refine loop: analyze a brownfield run, apply safe fixes, optional agent pass, re-grade.
# Usage: sh refine_loop.sh <repo-dir> <log-dir> [rounds]
set -eu

REPO=$1
LOGS=$2
ROUNDS=${3:-2}
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/common.sh"
ensure_bench_wteam
WTEAM=$(wteam_bin)
require_cmd agent python3

r=0
while [ "$r" -lt "$ROUNDS" ]; do
  r=$((r + 1))
  echo "=== refine round $r ==="
  "$WTEAM" --root "$REPO" --slug bench refine \
    --logs "$LOGS" \
    --grade "$LOGS/grade.txt" \
    --run-log "$LOGS/run.txt" \
    --apply >"$LOGS/refine.$r.txt" 2>&1 || true
  tail -n 40 "$LOGS/refine.$r.txt"

  if sh "$HERE/grade.sh" "$REPO" >"$LOGS/grade.$r.txt" 2>&1; then
    echo "grade_pass 1 (round $r)"
    tail -5 "$LOGS/grade.$r.txt"
    exit 0
  fi
  echo "grade still failing (round $r)"

  PROMPT="$(cat "$HERE/../../skills/refine/SKILL.md" 2>/dev/null | sed -n '/^## Agent pass/,$p' | head -n 40)
Repo: $REPO
Logs: $LOGS
Refine report:
$(cat "$LOGS/refine.$r.txt")

Apply the minimum changes to address priority-1 recommendations. Use the checkout wteam on PATH (current release). Land work on main, not only worktrees. Run cargo fmt --check and cargo test before finishing."
  timeout 1800 agent -p --output-format stream-json --trust --force \
    --model "${WTEAM_BENCH_MODEL:-composer-2.5}" --workspace "$REPO" "$PROMPT" \
    >"$LOGS/refine-agent.$r.jsonl" 2>"$LOGS/refine-agent.$r.stderr.log" || true
done

sh "$HERE/grade.sh" "$REPO" >"$LOGS/grade.final.txt" 2>&1 || {
  tail -15 "$LOGS/grade.final.txt"
  exit 1
}
tail -5 "$LOGS/grade.final.txt"

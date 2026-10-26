#!/bin/sh
# Blindly compare two passing outputs. The caller controls A/B order.
# Usage: sh judge.sh <A-directory> <B-directory> <output-jsonl>
set -eu

A=$1
B=$2
OUT=$3
HERE=$(cd "$(dirname "$0")" && pwd)
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
MODEL=${WTEAM_BENCH_MODEL:-composer-2.5}

mkdir -p "$TMP/A" "$TMP/B"
# Diff from the brownfield base commit (first commit) so landed tickets show up.
base_diff() {
  REPO=$1
  DEST=$2
  BASE=$(git -C "$REPO" rev-list --max-parents=0 HEAD | tail -n 1)
  git -C "$REPO" diff --no-ext-diff "$BASE"..HEAD >"$DEST/change.diff"
  # Also keep a file list for empty-diff debugging.
  git -C "$REPO" diff --stat "$BASE"..HEAD >"$DEST/stat.txt"
}
base_diff "$A" "$TMP/A"
base_diff "$B" "$TMP/B"
cp "$HERE/task.md" "$TMP/task.md"

PROMPT='Compare A/change.diff and B/change.diff as a blind senior-maintainer review. Both candidates were given task.md and must already pass the same hidden tests. Do not infer their origin. Score each candidate from 1-5 on modularity/cohesion, duplication, error handling, ease of local reasoning, and test quality. For every score, cite a concrete file and symbol or test. Choose A, B, or tie overall. Penalize needless code and speculative abstractions. Return a concise Markdown table followed by the overall choice and two-sentence rationale.'
agent -p --output-format stream-json --trust --force --mode ask \
  --model "$MODEL" --workspace "$TMP" "$PROMPT" >"$OUT"

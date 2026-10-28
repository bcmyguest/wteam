#!/bin/sh
# One agent session on the full TB instruction (harness continuous arm).
set -eu
INSTRUCTION=$1
MODEL=${BENCH_MODEL:-auto}
CLI=${BENCH_CLI:-cursor}
cd /app
export PATH="/usr/local/bin:/opt/cursor-agent:${PATH:-/usr/bin:/bin}"
export HOME="${HOME:-/root}"

case $CLI in
  codex)
    exec codex exec --json --dangerously-bypass-approvals-and-sandbox \
      --skip-git-repo-check --model "$MODEL" --cd /app -- "$INSTRUCTION"
    ;;
  cursor)
    exec agent -p --trust --force --output-format stream-json \
      --model "$MODEL" --workspace /app "$INSTRUCTION"
    ;;
  opencode)
    # Prefer lemonade/<id>; bare ids get the lemonade/ prefix for local Lemonade.
    case $MODEL in
      */*) OC_MODEL=$MODEL ;;
      *) OC_MODEL="lemonade/$MODEL" ;;
    esac
    exec opencode run --format json --dangerously-skip-permissions \
      -m "$OC_MODEL" -- "$INSTRUCTION"
    ;;
  *) echo "unsupported BENCH_CLI=$CLI" >&2; exit 2 ;;
esac

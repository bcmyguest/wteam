#!/bin/sh
# One agent session on the full TB instruction (harness continuous arm).
set -eu
INSTRUCTION=$1
MODEL=${BENCH_MODEL:-auto}
cd /app
export PATH="/usr/local/bin:/opt/cursor-agent:${PATH:-/usr/bin:/bin}"
export HOME="${HOME:-/root}"

if ! command -v agent >/dev/null 2>&1; then
  echo "wteam-harness continuous: agent not on PATH" >&2
  ls -la /usr/local/bin /opt/cursor-agent 2>&1 || true
  exit 1
fi

exec agent -p --trust --force --output-format stream-json \
  --model "$MODEL" --workspace /app "$INSTRUCTION"

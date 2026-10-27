#!/bin/sh
# Resume the latest (or given) overnight run; skips successfully completed phases.
BASE=${1:-${OVERNIGHT_BASE:-}}
if [ -z "$BASE" ] && [ -L /tmp/wteam-overnight-latest ]; then
  BASE=$(readlink -f /tmp/wteam-overnight-latest)
fi
if [ -z "$BASE" ] || [ ! -d "$BASE" ]; then
  echo "usage: sh resume-overnight.sh [/tmp/wteam-overnight-...]" >&2
  exit 2
fi
export OVERNIGHT_BASE=$BASE
export RESUME=1
exec sh "$(cd "$(dirname "$0")" && pwd)/overnight.sh"

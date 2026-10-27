#!/bin/sh
# Usage: sh run.sh <continuous|fresh|team> <out-dir> <log-dir> [pairs]
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
mode=$1
shift
case $mode in
  continuous|single) exec sh "$HERE/run_continuous.sh" "$@" ;;
  fresh) exec sh "$HERE/run_fresh.sh" "$@" ;;
  team|wteam) exec sh "$HERE/run_team.sh" "$@" ;;
  *)
    echo "usage: sh run.sh <continuous|fresh|team> <out-dir> <log-dir> [pairs]" >&2
    exit 2
    ;;
esac

#!/bin/sh
# Usage: sh run.sh <continuous|fresh|team> ...
set -eu
RUNNER=$(cd "$(dirname "$0")" && pwd)
mode=$1
shift
case $mode in
  continuous|single) exec sh "$RUNNER/run_continuous.sh" "$@" ;;
  fresh) exec sh "$RUNNER/run_fresh.sh" "$@" ;;
  team|wteam) exec sh "$RUNNER/run_team.sh" "$@" ;;
  *)
    echo "usage: sh run.sh <continuous|fresh|team> <lib> <work-dir> …" >&2
    exit 2
    ;;
esac

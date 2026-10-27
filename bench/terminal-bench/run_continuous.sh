#!/bin/sh
# One agent session per task (Harbor default) — maps to bench/runner "continuous".
# Usage: sh run_continuous.sh [n_tasks] [extra harbor args...]
# Env: TB_MODEL, TB_AGENT (default cursor-cli), TB_DATASET, TB_INCLUDE_TASK (-i name)
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/common.sh"
N=${1:-5}
shift || true
extra=
[ -n "${TB_INCLUDE_TASK:-}" ] && extra="$extra -i $TB_INCLUDE_TASK"
# shellcheck disable=SC2086
harbor_run continuous -l "$N" $extra "$@"

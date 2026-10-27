#!/bin/sh
# TB tasks, harness team arm (wteam run per task).
# Usage: sh run_team.sh [n_tasks]
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/common.sh"
N=${1:-1}
shift || true
TB_N_ATTEMPTS=1
export TB_N_ATTEMPTS
extra=
[ -n "${TB_INCLUDE_TASK:-}" ] && extra="$extra -i $TB_INCLUDE_TASK"
# shellcheck disable=SC2086
harness_harbor_run team -l "$N" $extra "$@"

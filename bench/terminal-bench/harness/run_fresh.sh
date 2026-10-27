#!/bin/sh
# TB tasks, harness fresh arm (independent attempts per task).
# Usage: sh run_fresh.sh [n_tasks]
# Env: TB_FRESH_ATTEMPTS (default 3)
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/common.sh"
N=${1:-1}
shift || true
export TB_N_ATTEMPTS=${TB_FRESH_ATTEMPTS:-3}
extra=
[ -n "${TB_INCLUDE_TASK:-}" ] && extra="$extra -i $TB_INCLUDE_TASK"
# shellcheck disable=SC2086
harness_harbor_run fresh -l "$N" $extra "$@"

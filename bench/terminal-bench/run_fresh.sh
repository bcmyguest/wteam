#!/bin/sh
# Independent attempt per trial (no session carryover between attempts).
# Usage: sh run_fresh.sh [n_tasks]
# Sets TB_N_ATTEMPTS>1 only when TB_FRESH_ATTEMPTS is set; default 1 attempt = same as continuous per task.
# For strict "new session each attempt", use: TB_N_ATTEMPTS=3 sh run_fresh.sh 5
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/common.sh"
N=${1:-5}
export TB_N_ATTEMPTS=${TB_FRESH_ATTEMPTS:-${TB_N_ATTEMPTS:-1}}
harbor_run fresh -l "$N" ${TB_INCLUDE_TASK:+-i "$TB_INCLUDE_TASK"}

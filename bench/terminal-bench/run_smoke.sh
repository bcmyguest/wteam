#!/bin/sh
# Confirm dataset + containers: oracle solves first N tasks.
# Usage: sh run_smoke.sh [n_tasks]
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/common.sh"
N=${1:-1}
TB_AGENT=oracle harbor_run smoke -l "$N"

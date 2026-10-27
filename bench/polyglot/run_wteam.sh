#!/bin/sh
# wteam: one ticket per exercise (CLI via bench/runner).
#   sh run_wteam.sh <polyglot checkout> <work dir> <pairs> [model]
# Legacy: sh run_wteam.sh <checkout> <dir> <model> <pairs>
# Env: BENCH_CLI, BENCH_REVIEW=autopass|on, DISPATCH=auto|model
set -u
export BENCH_CLI=${BENCH_CLI:-cursor}
if [ $# -ge 4 ]; then
  BENCH_MODEL=$3
  export BENCH_MODEL
  set -- "$1" "$2" "$4"
fi
exec sh "$(cd "$(dirname "$0")" && pwd)/../runner/run_polyglot_team.sh" "$@"

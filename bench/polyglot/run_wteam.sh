#!/bin/sh
# wteam: one ticket per exercise (CLI via bench/runner).
#   sh run_wteam.sh <polyglot checkout> <work dir> <pairs> [model]
# Env: BENCH_CLI, BENCH_REVIEW=autopass|on, DISPATCH=auto|model, REVIEWER via BENCH_REVIEWER_MODEL
set -u
export BENCH_CLI=${BENCH_CLI:-cursor}
exec sh "$(cd "$(dirname "$0")" && pwd)/../runner/run_polyglot_team.sh" "$@"

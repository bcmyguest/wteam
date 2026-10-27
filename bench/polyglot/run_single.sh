#!/bin/sh
# Baseline: ONE headless session for all exercises (CLI via bench/runner).
#   sh run_single.sh <polyglot checkout> <work dir> [model]
# Env: BENCH_CLI (default cursor), HIDDEN=1, BENCH_LIMIT=N
set -u
export BENCH_CLI=${BENCH_CLI:-cursor}
exec sh "$(cd "$(dirname "$0")" && pwd)/../runner/run_polyglot_continuous.sh" "$@"

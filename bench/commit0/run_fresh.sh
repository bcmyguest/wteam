#!/bin/sh
# One fresh session per ticket (delegates to bench/runner).
#   sh run_fresh.sh <lib> <work dir> [model]
set -u
export BENCH_CLI=${BENCH_CLI:-claude}
exec sh "$(cd "$(dirname "$0")" && pwd)/../runner/run_fresh.sh" "$@"

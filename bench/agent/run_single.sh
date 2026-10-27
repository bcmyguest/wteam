#!/bin/sh
# Delegates to bench/runner (continuous mode). Default CLI: cursor.
export BENCH_CLI=${BENCH_CLI:-cursor}
exec sh "$(cd "$(dirname "$0")" && pwd)/../runner/run_continuous.sh" "$@"

#!/bin/sh
export BENCH_CLI=${BENCH_CLI:-cursor}
exec sh "$(cd "$(dirname "$0")" && pwd)/../runner/run_fresh.sh" "$@"

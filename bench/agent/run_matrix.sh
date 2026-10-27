#!/bin/sh
export BENCH_CLI=${BENCH_CLI:-cursor}
export BENCH_LIBS=${AGENT_BENCH_LIBS:-${BENCH_LIBS:-incidentkit-debug tinydb}}
exec sh "$(cd "$(dirname "$0")" && pwd)/../runner/run_matrix.sh" "$@"

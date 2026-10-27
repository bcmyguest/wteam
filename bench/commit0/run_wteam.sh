#!/bin/sh
# wteam planner + pairs (delegates to bench/runner; same args as before).
#   sh run_wteam.sh <lib> <work dir> <planner> <worker> <reviewer> <pairs>
# Env: REVIEW=off → autopass reviewer; CODEGRAPH, SHAPE, RUN_TIMEOUT, OVERLAP unchanged.
set -u
export BENCH_CLI=${BENCH_CLI:-claude}
if [ "${REVIEW:-on}" = off ]; then
  export BENCH_REVIEW=autopass
else
  export BENCH_REVIEW=${BENCH_REVIEW:-on}
fi
exec sh "$(cd "$(dirname "$0")" && pwd)/../runner/run_team.sh" "$@"

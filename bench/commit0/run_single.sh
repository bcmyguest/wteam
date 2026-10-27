#!/bin/sh
# Baseline: ONE headless session rebuilds the whole library (delegates to bench/runner).
#   sh run_single.sh <lib> <work dir> <model>
# Env: PLAN=<dir> — same session, plan embedded in prompt.
set -u
export BENCH_CLI=${BENCH_CLI:-claude}
exec sh "$(cd "$(dirname "$0")" && pwd)/../runner/run_continuous.sh" "$@"

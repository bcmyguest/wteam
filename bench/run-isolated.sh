#!/bin/sh
# Run a command without the interactive shell profile (avoids huge state dumps in agent sessions).
# Usage: sh bench/run-isolated.sh command [args...]
if [ $# -lt 1 ]; then
  echo "usage: run-isolated.sh command [args...]" >&2
  exit 2
fi
exec /usr/bin/env -i \
  "HOME=${HOME}" \
  "USER=${USER:-$(id -un 2>/dev/null || echo "")}" \
  "LOGNAME=${LOGNAME:-$(id -un 2>/dev/null || echo "")}" \
  "LANG=${LANG:-C.UTF-8}" \
  "TERM=${TERM:-dumb}" \
  "PATH=${PATH:-/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin}" \
  "WTEAM=${WTEAM:-}" \
  "BENCH_CLI=${BENCH_CLI:-}" \
  "BENCH_MODEL=${BENCH_MODEL:-}" \
  "CURSOR_API_KEY=${CURSOR_API_KEY:-}" \
  "TB_AGENT=${TB_AGENT:-}" \
  "TB_MODEL=${TB_MODEL:-}" \
  "TB_JOBS_DIR=${TB_JOBS_DIR:-}" \
  "TB_DATASET=${TB_DATASET:-}" \
  "TB_MOUNT_CURSOR_AUTH=${TB_MOUNT_CURSOR_AUTH:-}" \
  "TB_SKIP_CREDENTIAL_CHECKS=${TB_SKIP_CREDENTIAL_CHECKS:-}" \
  "TB_JOB_PREFIX=${TB_JOB_PREFIX:-}" \
  "TB_RUN_BASE=${TB_RUN_BASE:-}" \
  "TB_HARNESS_DIR=${TB_HARNESS_DIR:-}" \
  "PYTHONPATH=${PYTHONPATH:-}" \
  "BENCH_REVIEW=${BENCH_REVIEW:-}" \
  /bin/sh -c 'exec "$@"' _ "$@"

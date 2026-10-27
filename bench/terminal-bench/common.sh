# Terminal-Bench 4.0 via Harbor (66 public tasks on Harbor Hub).
# shellcheck shell=sh
TB_BENCH=$(cd "$(dirname "$0")" && pwd)
# Harbor Hub id for TB 4.0 (66 tasks) — same set as tbench.ai agent leaderboard.
export TB_DATASET=${TB_DATASET:-terminal-bench/terminal-bench}
export TB_JOBS_DIR=${TB_JOBS_DIR:-$TB_BENCH/jobs}
export TB_AGENT=${TB_AGENT:-cursor-cli}
export TB_MODEL=${TB_MODEL:-${BENCH_MODEL:-composer-2.5}}
export TB_N_CONCURRENT=${TB_N_CONCURRENT:-1}
export TB_N_ATTEMPTS=${TB_N_ATTEMPTS:-1}

require_harbor() {
  command -v harbor >/dev/null 2>&1 || {
    echo "install: uv tool install harbor" >&2
    exit 1
  }
}

require_docker() {
  docker info >/dev/null 2>&1 || {
    echo "Docker required for local Harbor runs (or set TB_ENV=modal/daytona + keys)" >&2
    exit 1
  }
}

harbor_run() {
  mode=$1
  shift
  require_harbor
  require_docker
  mkdir -p "$TB_JOBS_DIR"
  job_name="${TB_JOB_PREFIX:-tb}-${mode}-$(date +%Y%m%d-%H%M%S)"
  set -- harbor run \
    -d "$TB_DATASET" \
    -a "$TB_AGENT" \
    -o "$TB_JOBS_DIR" \
    --job-name "$job_name" \
    -n "$TB_N_CONCURRENT" \
    -k "$TB_N_ATTEMPTS" \
    ${TB_ENV:+--env "$TB_ENV"} \
    "$@"
  if [ "$TB_AGENT" != oracle ]; then
    set -- "$@" -m "$TB_MODEL"
  fi
  # shellcheck disable=SC2086
  "$@"
}

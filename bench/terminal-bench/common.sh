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

# Optional: bench/terminal-bench/.env.local or ~/.config/cursor/harbor.env (gitignored)
load_tb_env() {
  for f in "$TB_BENCH/.env.local" "${HOME:-}/.config/cursor/harbor.env"; do
    if [ -f "$f" ]; then
      set -a
      # shellcheck disable=SC1090
      . "$f"
      set +a
      return 0
    fi
  done
  return 1
}

cursor_auth_file() {
  printf '%s\n' "${TB_CURSOR_AUTH_FILE:-${HOME:-}/.config/cursor/auth.json}"
}

warn_harbor_cursor_auth() {
  [ "$TB_AGENT" = cursor-cli ] || return 0
  load_tb_env 2>/dev/null || true
  if [ -n "${CURSOR_API_KEY:-}" ]; then
    return 0
  fi
  auth=$(cursor_auth_file)
  if [ -f "$auth" ]; then
    echo "note: mounting $(cursor_auth_file) into Harbor (OAuth from agent login)." >&2
    echo "      Stock cursor-cli may still require CURSOR_API_KEY — see README." >&2
    return 0
  fi
  echo "note: Harbor cursor-cli in Docker; using --skip-credential-checks." >&2
  echo "      Set CURSOR_API_KEY in .env.local only if trials fail (see env.example)." >&2
}

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
  load_tb_env 2>/dev/null || true
  require_harbor
  require_docker
  warn_harbor_cursor_auth
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
  if [ "$TB_AGENT" = cursor-cli ]; then
    case ${TB_MOUNT_CURSOR_AUTH:-auto} in
      0|false|no|off) ;;
      *)
        auth=$(cursor_auth_file)
        if [ -f "$auth" ]; then
          mounts='[{"type":"bind","source":"'"$auth"'","target":"/root/.config/cursor/auth.json","read_only":true}]'
          set -- "$@" --mounts "$mounts"
        fi
        ;;
    esac
  fi
  # Harbor preflight wants CURSOR_API_KEY; local `agent` may already be logged in.
  if [ "$TB_AGENT" = cursor-cli ] && [ -z "${CURSOR_API_KEY:-}" ]; then
    case ${TB_SKIP_CREDENTIAL_CHECKS:-auto} in
      1|true|yes|auto) set -- "$@" --skip-credential-checks ;;
    esac
  fi
  # shellcheck disable=SC2086
  "$@"
}

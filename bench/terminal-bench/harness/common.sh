# harness-tb: Harbor trials with wteam-harness agent (TB tasks, our context arms).
# shellcheck shell=sh
HARNESS=$(cd "$(dirname "$0")" && pwd)
TB_BENCH=$(cd "$HARNESS/.." && pwd)
REPO=$(cd "$HARNESS/../../.." && pwd)
. "$TB_BENCH/common.sh"

export TB_HARNESS_AGENT=${TB_HARNESS_AGENT:-wteam_harbor_agent:HarnessAgent}
export TB_HARNESS_DIR=${TB_HARNESS_DIR:-/tmp/wteam-harness-tb-$(date +%Y%m%d-%H%M%S)}
export PYTHONPATH="${HARNESS}${PYTHONPATH:+:$PYTHONPATH}"

require_harness_binaries() {
  require_harbor
  require_docker
  if [ -z "${WTEAM:-}" ] || [ ! -x "$WTEAM" ]; then
    if [ -x "$REPO/target/release/wteam" ]; then
      WTEAM=$REPO/target/release/wteam
      export WTEAM
    else
      echo "build wteam: cargo build --release (or set WTEAM=)" >&2
      exit 1
    fi
  fi
  AGENT_BIN=${TB_AGENT_BIN:-${WTEAM_CURSOR_BIN:-agent}}
  if ! command -v "$AGENT_BIN" >/dev/null 2>&1; then
    echo "host agent not on PATH: set TB_AGENT_BIN or install Cursor agent CLI" >&2
    exit 1
  fi
  AGENT_BIN=$(command -v "$AGENT_BIN")
  # Resolve through symlinks: ~/.local/bin/agent → …/versions/<ver>/cursor-agent
  # Mount the whole version dir (node + index.js); a lone wrapper dies in ~100ms inside Docker.
  if command -v realpath >/dev/null 2>&1; then
    AGENT_REAL=$(realpath "$AGENT_BIN")
  else
    AGENT_REAL=$(readlink -f "$AGENT_BIN" 2>/dev/null || echo "$AGENT_BIN")
  fi
  AGENT_DIR=$(dirname "$AGENT_REAL")
  if [ ! -x "$AGENT_DIR/node" ] || [ ! -f "$AGENT_DIR/index.js" ]; then
    echo "cursor-agent install incomplete under $AGENT_DIR (need node + index.js)" >&2
    exit 1
  fi
  export AGENT_BIN AGENT_DIR AGENT_REAL
  AUTH=$(cursor_auth_file)
  if [ ! -f "$AUTH" ]; then
    echo "missing $AUTH — run agent login on the host" >&2
    exit 1
  fi
}

harness_mounts_json() {
  auth=$(cursor_auth_file)
  # Full cursor-agent package dir — NOT the thin ~/.local/bin/agent symlink/wrapper alone.
  printf '[{"type":"bind","source":"%s","target":"/root/.config/cursor/auth.json","read_only":true},' "$auth"
  printf '{"type":"bind","source":"%s","target":"/usr/local/bin/wteam","read_only":true},' "$WTEAM"
  printf '{"type":"bind","source":"%s","target":"/opt/cursor-agent","read_only":true},' "$AGENT_DIR"
  printf '{"type":"bind","source":"%s","target":"/opt/wteam-harness/in_container_continuous.sh","read_only":true},' "$HARNESS/in_container_continuous.sh"
  printf '{"type":"bind","source":"%s","target":"/opt/wteam-harness/in_container_team.sh","read_only":true}]' "$HARNESS/in_container_team.sh"
}

# Usage: harness_harbor_run <continuous|fresh|team> -l N [extra harbor args...]
harness_harbor_run() {
  mode=$1
  shift
  require_harness_binaries
  load_tb_env 2>/dev/null || true
  mkdir -p "$TB_HARNESS_DIR/jobs"
  job_name="${TB_JOB_PREFIX:-tb-harness}-${mode}-$(date +%Y%m%d-%H%M%S)"
  mounts=$(harness_mounts_json)
  set -- harbor run \
    -d "$TB_DATASET" \
    -a "$TB_HARNESS_AGENT" \
    -o "$TB_HARNESS_DIR/jobs" \
    --job-name "$job_name" \
    -n "$TB_N_CONCURRENT" \
    -k "$TB_N_ATTEMPTS" \
    --mounts "$mounts" \
    --ak "mode=$mode" \
    --ak "model=${TB_MODEL:-${BENCH_MODEL:-auto}}" \
    ${TB_ENV:+--env "$TB_ENV"} \
    "$@"
  if [ -n "${TB_MODEL:-}" ] || [ -n "${BENCH_MODEL:-}" ]; then
    set -- "$@" -m "${TB_MODEL:-${BENCH_MODEL:-auto}}"
  fi
  if [ -z "${CURSOR_API_KEY:-}" ]; then
    case ${TB_SKIP_CREDENTIAL_CHECKS:-auto} in
      1|true|yes|auto) set -- "$@" --skip-credential-checks ;;
    esac
  fi
  echo "harness-tb mode=$mode job=$job_name dir=$TB_HARNESS_DIR agent_dir=$AGENT_DIR" >&2
  # shellcheck disable=SC2086
  "$@"
}

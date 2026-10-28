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
  HARNESS_CLI=${BENCH_CLI:-cursor}
  case $HARNESS_CLI in
    codex)
      CODEX_BIN=${TB_CODEX_BIN:-$(command -v codex)}
      CODEX_BIN=$(realpath "$CODEX_BIN")
      CODEX_HOST_BIN=$(dirname "$CODEX_BIN")/codex-code-mode-host
      AUTH=${TB_CODEX_AUTH_FILE:-${CODEX_HOME:-$HOME/.codex}/auth.json}
      [ -x "$CODEX_BIN" ] || { echo "missing Codex binary: $CODEX_BIN" >&2; exit 1; }
      [ -x "$CODEX_HOST_BIN" ] || { echo "missing Codex code-mode host: $CODEX_HOST_BIN" >&2; exit 1; }
      [ -f "$AUTH" ] || { echo "missing Codex auth: $AUTH" >&2; exit 1; }
      export CODEX_BIN CODEX_HOST_BIN
      ;;
    cursor)
      AGENT_BIN=${TB_AGENT_BIN:-${WTEAM_CURSOR_BIN:-agent}}
      if ! command -v "$AGENT_BIN" >/dev/null 2>&1; then
        echo "host agent not on PATH: set TB_AGENT_BIN or install Cursor agent CLI" >&2
        exit 1
      fi
      AGENT_BIN=$(command -v "$AGENT_BIN")
      # Mount the whole version dir (node + index.js); a lone wrapper dies in ~100ms.
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
      AUTH=$(cursor_auth_file)
      [ -f "$AUTH" ] || { echo "missing Cursor auth: $AUTH" >&2; exit 1; }
      export AGENT_BIN AGENT_DIR AGENT_REAL
      ;;
    opencode)
      OPENCODE_BIN=${TB_OPENCODE_BIN:-$(command -v opencode)}
      OPENCODE_BIN=$(realpath "$OPENCODE_BIN")
      [ -x "$OPENCODE_BIN" ] || { echo "missing opencode binary: $OPENCODE_BIN" >&2; exit 1; }
      LEMONADE_HOST=${TB_LEMONADE_HOST:-172.17.0.1}
      LEMONADE_PORT=${TB_LEMONADE_PORT:-13305}
      LEMONADE_BASE_URL=${TB_LEMONADE_BASE_URL:-http://${LEMONADE_HOST}:${LEMONADE_PORT}/v1}
      MODEL_NAME=${TB_MODEL:-${BENCH_MODEL:-lemonade/Nemotron-3.5-Bolt-E4B-GGUF}}
      # Strip provider prefix for lemonade model id inside opencode.json.
      MODEL_ID=${MODEL_NAME#lemonade/}
      MODEL_ID=${MODEL_ID#openai/}
      OPENCODE_CFG_DIR=${TB_HARNESS_DIR:-/tmp/wteam-harness-tb}/opencode-config
      mkdir -p "$OPENCODE_CFG_DIR"
      OPENCODE_CFG=$OPENCODE_CFG_DIR/opencode.json
      AUTH=$OPENCODE_CFG
      cat >"$OPENCODE_CFG" <<EOF
{
  "provider": {
    "lemonade": {
      "npm": "@ai-sdk/openai-compatible",
      "name": "Lemonade",
      "models": {
        "$MODEL_ID": { "tool_call": true }
      },
      "options": {
        "baseURL": "$LEMONADE_BASE_URL",
        "apiKey": "{env:OPENAI_API_KEY}"
      }
    }
  }
}
EOF
      HOST_GATEWAY_COMPOSE=${TB_HOST_GATEWAY_COMPOSE:-$OPENCODE_CFG_DIR/host-gateway.compose.yaml}
      cat >"$HOST_GATEWAY_COMPOSE" <<'YAML'
services:
  main:
    extra_hosts:
      - "host.docker.internal:host-gateway"
YAML
      export OPENCODE_BIN LEMONADE_BASE_URL MODEL_NAME MODEL_ID OPENCODE_CFG HOST_GATEWAY_COMPOSE
      export OPENAI_API_KEY=${OPENAI_API_KEY:-lemonade}
      ;;
    *) echo "unsupported BENCH_CLI=$HARNESS_CLI (use codex, cursor, or opencode)" >&2; exit 2 ;;
  esac
  export HARNESS_CLI AUTH
}

harness_mounts_json() {
  case $HARNESS_CLI in
    codex)
      printf '[{"type":"bind","source":"%s","target":"/opt/wteam-harness/auth.json","read_only":true},' "$AUTH"
      printf '{"type":"bind","source":"%s","target":"/usr/local/bin/codex","read_only":true},' "$CODEX_BIN"
      printf '{"type":"bind","source":"%s","target":"/usr/local/bin/codex-code-mode-host","read_only":true},' "$CODEX_HOST_BIN"
      ;;
    cursor)
      printf '[{"type":"bind","source":"%s","target":"/root/.config/cursor/auth.json","read_only":true},' "$AUTH"
      printf '{"type":"bind","source":"%s","target":"/opt/cursor-agent","read_only":true},' "$AGENT_DIR"
      ;;
    opencode)
      printf '[{"type":"bind","source":"%s","target":"/opt/wteam-harness/opencode.json","read_only":true},' "$OPENCODE_CFG"
      printf '{"type":"bind","source":"%s","target":"/usr/local/bin/opencode","read_only":true},' "$OPENCODE_BIN"
      ;;
  esac
  printf '{"type":"bind","source":"%s","target":"/usr/local/bin/wteam","read_only":true},' "$WTEAM"
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
    --ak "cli=$HARNESS_CLI" \
    --ak "model=${TB_MODEL:-${BENCH_MODEL:-auto}}" \
    --ak "pairs=${TB_PAIRS:-4}" \
    --ak "review=${BENCH_REVIEW:-autopass}" \
    ${TB_ENV:+--env "$TB_ENV"} \
    "$@"
  if [ -n "${TB_MODEL:-}" ] || [ -n "${BENCH_MODEL:-}" ]; then
    set -- "$@" -m "${TB_MODEL:-${BENCH_MODEL:-auto}}"
  fi
  if [ "$HARNESS_CLI" = opencode ]; then
    set -- "$@" \
      --extra-docker-compose "$HOST_GATEWAY_COMPOSE" \
      --ae "OPENAI_API_KEY=${OPENAI_API_KEY:-lemonade}" \
      --ak "lemonade_base_url=$LEMONADE_BASE_URL"
  fi
  if [ -z "${CURSOR_API_KEY:-}" ]; then
    case ${TB_SKIP_CREDENTIAL_CHECKS:-auto} in
      1|true|yes|auto) set -- "$@" --skip-credential-checks ;;
    esac
  fi
  echo "harness-tb mode=$mode cli=$HARNESS_CLI job=$job_name dir=$TB_HARNESS_DIR" >&2
  # shellcheck disable=SC2086
  "$@"
}

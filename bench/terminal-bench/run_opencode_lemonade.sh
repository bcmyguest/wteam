#!/bin/sh
# Terminal-Bench via Harbor OpenCode + local Lemonade (Linux Docker).
#
# Usage:
#   sh run_opencode_lemonade.sh [n_tasks] [model_id]
#   TB_MODE=harness sh run_opencode_lemonade.sh 1 Qwen3.6-35B-A3B-ThinkingCoder
#   TB_MODE=harness-team sh run_opencode_lemonade.sh 1
#
# Env: TB_LEMONADE_HOST (default 172.17.0.1), TB_LEMONADE_PORT (13305),
#      TB_EXCLUDE_LAYOUT=1 (default; skips vision layout task), OPENAI_API_KEY,
#      TB_OPENCODE_ENABLE_THINKING (default 0 — required for tool-calling TB),
#      TB_LEMONADE_LOAD=1 (default; POST /api/v1/load before Harbor)
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
N=${1:-3}
MODEL_ID=${2:-Qwen3.6-35B-A3B-ThinkingCoder}
HOST_IP=${TB_LEMONADE_HOST:-172.17.0.1}
PORT=${TB_LEMONADE_PORT:-13305}
BASE_URL="http://${HOST_IP}:${PORT}/v1"
MODE=${TB_MODE:-external}
OVERLAY=${TB_HOST_GATEWAY_COMPOSE:-$HERE/.cache/host-gateway.compose.yaml}
mkdir -p "$(dirname "$OVERLAY")"
cat > "$OVERLAY" <<YAML
services:
  main:
    extra_hosts:
      - "host.docker.internal:host-gateway"
YAML
export OPENAI_API_KEY=${OPENAI_API_KEY:-lemonade}
export TB_MODEL="lemonade/${MODEL_ID}"
export BENCH_MODEL="$TB_MODEL"
export TB_N_CONCURRENT=${TB_N_CONCURRENT:-1}
export TB_HOST_GATEWAY_COMPOSE=$OVERLAY
export TB_LEMONADE_HOST=$HOST_IP
export TB_LEMONADE_PORT=$PORT
export TB_LEMONADE_BASE_URL=$BASE_URL
export TB_OPENCODE_ENABLE_THINKING=${TB_OPENCODE_ENABLE_THINKING:-0}

# Ensure the requested Lemonade model is loaded (host loopback, not docker IP).
# ThinkingCoder's saved recipe forces preserve_thinking + <think> generation
# prompts, which burns OpenCode's entire output budget before any tool call.
# Default load strips that for TB (override with TB_LEMONADE_LOAD_ARGS=).
case ${TB_LEMONADE_LOAD:-1} in
  0|false|no|off) ;;
  *)
    LOAD_URL=${TB_LEMONADE_LOAD_URL:-http://127.0.0.1:${PORT}/api/v1/load}
    UNLOAD_URL=${TB_LEMONADE_UNLOAD_URL:-http://127.0.0.1:${PORT}/api/v1/unload}
    echo "lemonade unload+load model_name=$MODEL_ID" >&2
    curl -sS -m 120 -X POST "$UNLOAD_URL" \
      -H 'Content-Type: application/json' \
      -d "{\"model_name\":\"$MODEL_ID\"}" >/tmp/lemonade-unload-$MODEL_ID.json 2>/dev/null || true
    # Python builds the load body so ThinkingCoder chat-template kwargs stay valid JSON.
    if ! python3 - "$LOAD_URL" "$MODEL_ID" \
      >/tmp/lemonade-load-$MODEL_ID.json \
      2>/tmp/lemonade-load-$MODEL_ID.err <<'PY'
import json, os, sys, urllib.request

url, model = sys.argv[1], sys.argv[2]
# Empty TB_LEMONADE_LOAD_ARGS means "no override"; unset means apply Thinking defaults.
if "TB_LEMONADE_LOAD_ARGS" in os.environ:
    args = os.environ.get("TB_LEMONADE_LOAD_ARGS") or None
elif "ThinkingCoder" in model or "Thinking" in model:
    args = (
        "--temp 0.6 --top-p 0.95 --top-k 20 --min-p 0.00 "
        "--presence-penalty 0 --repeat-penalty 1.0 -b 1024 -ub 256 --parallel 1 "
        "--chat-template-kwargs "
        "'{\"enable_thinking\": false, \"preserve_thinking\": false}'"
    )
else:
    args = None

body = {"model_name": model, "save_options": False}
if args:
    body["ctx_size"] = int(os.environ.get("TB_LEMONADE_CTX_SIZE", "131072"))
    body["llamacpp_args"] = args
req = urllib.request.Request(
    url,
    data=json.dumps(body).encode(),
    headers={"Content-Type": "application/json"},
    method="POST",
)
try:
    with urllib.request.urlopen(req, timeout=300) as r:
        sys.stdout.write(r.read().decode())
except Exception as e:
    sys.stdout.write(json.dumps({"status": "error", "error": str(e)}))
    raise SystemExit(1)
PY
    then
      echo "warning: lemonade load failed (see /tmp/lemonade-load-$MODEL_ID.json)" >&2
    fi
    ;;
esac

# Hub dataset task ids are prefixed; unprefixed -x/-i do not match.
extra=
case ${TB_EXCLUDE_LAYOUT:-1} in
  0|false|no|off) ;;
  *) extra="$extra -x terminal-bench/layout-config-recreation2" ;;
esac
# Prefer an explicit include. Default smoke used to hit photonic (-l 1), which
# is a hard geometry optimizer — local models often write a file and still
# score 0. Override with TB_INCLUDE_TASK / TB_KEEP_DATASET_ORDER=1.
if [ -z "${TB_INCLUDE_TASK:-}" ] && [ -z "${TB_KEEP_DATASET_ORDER:-}" ] && [ "$N" -eq 1 ]; then
  export TB_INCLUDE_TASK=terminal-bench/react-lead-form
fi
# harness/run_*.sh adds -i from TB_INCLUDE_TASK; external path needs it here.
if [ "$MODE" = external ] || [ "$MODE" = "" ]; then
  [ -n "${TB_INCLUDE_TASK:-}" ] && extra="$extra -i $TB_INCLUDE_TASK"
fi
# Default MODE is external when unset above — already set earlier as TB_MODE:-external.
# Remaining args after n + model (optional); allow `shift` when only N was passed.
if [ "$#" -ge 2 ]; then
  shift 2
elif [ "$#" -ge 1 ]; then
  shift 1
fi

case $MODE in
  harness|harness-continuous)
    export BENCH_CLI=opencode
    # shellcheck disable=SC2086
    exec sh "$HERE/harness/run_continuous.sh" "$N" $extra "$@"
    ;;
  harness-team|team)
    export BENCH_CLI=opencode
    # shellcheck disable=SC2086
    exec sh "$HERE/harness/run_team.sh" "$N" $extra "$@"
    ;;
  external|*)
    export TB_AGENT=opencode
    OC_CTX=${TB_OPENCODE_CONTEXT:-131072}
    OC_OUT=${TB_OPENCODE_OUTPUT:-16384}
    case ${TB_OPENCODE_ENABLE_THINKING:-0} in
      1|true|yes|on) EXTRA='{}' ;;
      *) EXTRA='{"enable_thinking":false,"chat_template_kwargs":{"enable_thinking":false}}' ;;
    esac
    AK=$(printf '%s' "{\"provider\":{\"lemonade\":{\"npm\":\"@ai-sdk/openai-compatible\",\"name\":\"Lemonade\",\"models\":{\"$MODEL_ID\":{\"tool_call\":true,\"limit\":{\"context\":$OC_CTX,\"output\":$OC_OUT}}},\"options\":{\"baseURL\":\"$BASE_URL\",\"apiKey\":\"{env:OPENAI_API_KEY}\",\"extraBody\":$EXTRA}}}}")
    # shellcheck disable=SC2086
    exec sh "$HERE/run_continuous.sh" "$N" \
      --base-url "$BASE_URL" \
      --api-format openai_chat_completions \
      --extra-docker-compose "$OVERLAY" \
      --skip-credential-checks \
      --ae OPENAI_API_KEY="$OPENAI_API_KEY" \
      --ak "opencode_config=$AK" \
      $extra \
      "$@"
    ;;
esac

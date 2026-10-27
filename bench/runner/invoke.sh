# One headless agent turn (always a new session — no --resume).
# Usage: . invoke.sh && bench_invoke <workspace> <log-path> <prompt> [timeout_sec]
# shellcheck shell=sh

bench_invoke() {
  workspace=$1
  log=$2
  prompt=$3
  timeout_sec=${4:-7200}
  cli=$(bench_cli)
  model=$(bench_model)
  stderr="${log%.*}.stderr.log"
  case $cli in
    claude)
      timeout "$timeout_sec" claude -p "$prompt" \
        --model "$model" --output-format json \
        --setting-sources project,local --disable-slash-commands \
        --permission-mode dontAsk \
        --allowedTools "Read" "Edit" "Write" "Glob" "Grep" \
          "Bash(python3 *)" "Bash(pytest *)" "Bash(ls *)" "Bash(cat *)" "Bash(mkdir *)" \
          "Bash(wteam *)" "Bash(codegraph *)" \
        >"$log" 2>"$stderr"
      ;;
    cursor)
      agent_bin=${WTEAM_CURSOR_BIN:-agent}
      timeout "$timeout_sec" "$agent_bin" -p --output-format stream-json --trust --force \
        --model "$model" --workspace "$workspace" "$prompt" \
        >"$log" 2>"$stderr"
      ;;
    codex)
      timeout "$timeout_sec" codex exec --json --model "$model" \
        --cd "$workspace" "$prompt" \
        >"$log" 2>"$stderr"
      ;;
    fake)
      echo "fake bench invoke: set WTEAM_FAKE_DIR scripts if using wteam fake roles" >&2
      exit 2
      ;;
    *)
      echo "bench_invoke: unsupported BENCH_CLI=$cli" >&2
      exit 2
      ;;
  esac
}

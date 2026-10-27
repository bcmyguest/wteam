# Bench runner configuration (any wteam-supported CLI).
# shellcheck shell=sh
_script_dir=$(cd "$(dirname "$0")" && pwd)
if [ -f "$_script_dir/invoke.sh" ]; then
  RUNNER=$_script_dir
else
  RUNNER=$(cd "$_script_dir/../runner" && pwd)
fi
COMMIT0=$(cd "$RUNNER/../commit0" && pwd)
POLYGLOT=$(cd "$RUNNER/../polyglot" && pwd)
BF=$(cd "$RUNNER/../brownfield" && pwd)
. "$BF/common.sh"

bench_cli() {
  printf '%s\n' "${BENCH_CLI:-cursor}"
}

bench_default_model() {
  case $(bench_cli) in
    claude) printf '%s\n' haiku ;;
    cursor) printf '%s\n' auto ;;
    codex) printf '%s\n' "${BENCH_CODEX_MODEL:-}" ;;
    *) printf '%s\n' "${BENCH_MODEL:-default}" ;;
  esac
}

bench_model() {
  printf '%s\n' "${BENCH_MODEL:-$(bench_default_model)}"
}

bench_planner_model() {
  printf '%s\n' "${BENCH_PLANNER_MODEL:-$(bench_model)}"
}

bench_worker_model() {
  printf '%s\n' "${BENCH_WORKER_MODEL:-$(bench_model)}"
}

bench_reviewer_model() {
  printf '%s\n' "${BENCH_REVIEWER_MODEL:-$(bench_model)}"
}

bench_require_cli() {
  cli=$(bench_cli)
  case $cli in
    claude) require_cmd claude ;;
    cursor) require_cmd "${WTEAM_CURSOR_BIN:-agent}" ;;
    codex) require_cmd codex ;;
    fake) ;;
    *)
      echo "unknown BENCH_CLI=$cli (use claude, cursor, codex, fake, …)" >&2
      exit 2
      ;;
  esac
}

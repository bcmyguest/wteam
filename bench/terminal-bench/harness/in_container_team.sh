#!/bin/sh
# wteam team arm on a TB task: init + run with the Harbor instruction as the bench goal.
set -eu
INSTRUCTION=$1
SLUG=${WTEAM_SLUG:-tb}
MODEL=${BENCH_MODEL:-auto}
cd /app
export PATH="/usr/local/bin:/opt/cursor-agent:${PATH:-/usr/bin:/bin}"
export HOME="${HOME:-/root}"

if [ ! -d .git ]; then
  git init -q 2>/dev/null || true
  git add -A 2>/dev/null || true
  git commit -qm "harness-tb base" 2>/dev/null || true
fi

if ! command -v wteam >/dev/null 2>&1 || ! command -v agent >/dev/null 2>&1; then
  echo "wteam-harness: missing wteam or agent in container (check bind mounts)" >&2
  ls -la /usr/local/bin /opt/cursor-agent 2>&1 || true
  exit 1
fi

if [ ! -f ".wteam/$SLUG/state.json" ]; then
  wteam --root /app --slug "$SLUG" init
fi

mkdir -p ".wteam/bench"
printf '%s\n' "$INSTRUCTION" >".wteam/bench/task.md"
printf '%s\n' "Complete the Terminal-Bench task described in .wteam/bench/task.md. Work under /app. Verify with the task tests before finishing." \
  >".wteam/bench/planner_brief.txt"

mkdir -p ".wteam/agents"
for role in planner worker reviewer orchestrator; do
  f=".wteam/agents/$role.toml"
  if [ ! -f "$f" ]; then
    printf 'name = "%s"\ncli = "cursor"\nmodel = "%s"\nprompt = "%s"\nprofile = "worker"\nextra_args = ["--force"]\n' \
      "$role" "$MODEL" "$role" >"$f"
  fi
done

export WTEAM_BENCH_REVIEW=${WTEAM_BENCH_REVIEW:-autopass}
export WTEAM_BENCH_ISOLATION=${WTEAM_BENCH_ISOLATION:-none}
exec wteam --root /app --slug "$SLUG" run --timeout "${WTEAM_RUN_TIMEOUT:-28800}"

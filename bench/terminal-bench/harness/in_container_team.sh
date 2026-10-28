#!/bin/sh
# wteam team arm on a TB task: init + run with the Harbor instruction as the bench goal.
set -eu
INSTRUCTION=$1
SLUG=${WTEAM_SLUG:-tb}
MODEL=${BENCH_MODEL:-auto}
CLI=${BENCH_CLI:-cursor}
PAIRS=${WTEAM_PAIRS:-4}
REVIEW=${WTEAM_BENCH_REVIEW:-autopass}
cd /app
export PATH="/usr/local/bin:/opt/cursor-agent:${PATH:-/usr/bin:/bin}"
export HOME="${HOME:-/root}"

OC_MODEL=$MODEL
case $CLI in
  opencode)
    case $MODEL in
      */*) ;;
      *) OC_MODEL="lemonade/$MODEL" ;;
    esac
    ;;
esac

write_agent() {
  role=$1
  case $CLI in
    codex)
      printf 'name = "%s"\ncli = "codex"\nmodel = "%s"\nprompt = "%s"\nprofile = "full"\n' \
        "$role" "$MODEL" "$role" >".wteam/agents/$role.toml"
      ;;
    opencode)
      printf 'name = "%s"\ncli = "opencode"\nmodel = "%s"\nprompt = "%s"\nprofile = "full"\nextra_args = ["--dangerously-skip-permissions"]\n' \
        "$role" "$OC_MODEL" "$role" >".wteam/agents/$role.toml"
      ;;
    *)
      printf 'name = "%s"\ncli = "cursor"\nmodel = "%s"\nprompt = "%s"\nprofile = "worker"\nextra_args = ["--force"]\n' \
        "$role" "$MODEL" "$role" >".wteam/agents/$role.toml"
      ;;
  esac
}

command -v wteam >/dev/null 2>&1 || { echo "wteam missing in container" >&2; exit 1; }
case $CLI in
  codex) command -v codex >/dev/null 2>&1 || { echo "codex missing in container" >&2; exit 1; } ;;
  cursor) command -v agent >/dev/null 2>&1 || { echo "agent missing in container" >&2; exit 1; } ;;
  opencode) command -v opencode >/dev/null 2>&1 || { echo "opencode missing in container" >&2; exit 1; } ;;
  *) echo "unsupported BENCH_CLI=$CLI" >&2; exit 2 ;;
esac
if ! command -v git >/dev/null 2>&1; then
  if command -v apt-get >/dev/null 2>&1; then
    apt-get update -qq
    DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends git
  elif command -v apk >/dev/null 2>&1; then
    apk add --no-cache git
  elif command -v dnf >/dev/null 2>&1; then
    dnf install -y git
  elif command -v yum >/dev/null 2>&1; then
    yum install -y git
  else
    echo "git missing and no supported package manager in container" >&2
    exit 1
  fi
fi
if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  git init -q
fi
git config user.email bench@example.com
git config user.name bench
git add -A
git rev-parse HEAD >/dev/null 2>&1 || git commit --allow-empty -qm "chore: harness-tb base"

if [ ! -f ".wteam/$SLUG/state.json" ]; then
  wteam --root /app --slug "$SLUG" init
fi

printf '%s\n' "$INSTRUCTION" >".wteam/$SLUG/task.md"

mkdir -p ".wteam/agents"
for role in planner worker reviewer orchestrator; do
  write_agent "$role"
done
mkdir -p .wteam/roles
cat >.wteam/roles/worker.md <<'EOF'
# Terminal-Bench worker

Read your ticket and the full task in `.wteam/tb/task.md`. Work toward the task's requested artifact or system state. Run the checks available in the task environment. Hidden Harbor verifier tests are unavailable during your turn.

Submit the best completed attempt before ending your turn, even if a check still fails. State the failed check and remaining gap in `scratch/summary-<ticket>.md`, then call `wteam submit <ticket> --summary-file scratch/summary-<ticket>.md`. Include `--extra <path>` for changed files outside the ticket's `Files:` line. Use `wteam block` only when a missing prerequisite prevents any useful work.

Run one command per shell call. Write files with the file-editing tool, not shell redirection. Do not use git; wteam owns commits and landing.
EOF

case $REVIEW in
  autopass)
    mkdir -p .wteam-fake
    cat >.wteam-fake/reviewer.sh <<'EOF'
#!/bin/sh
set -eu
id=$(printf '%s\n' "$WTEAM_PROMPT" | sed -n 's/.*[Tt]icket \([0-9][0-9]\).*/\1/p' | head -1)
[ -n "$id" ] || { echo "no ticket id in prompt" >&2; exit 1; }
wteam verdict "$id" pass
EOF
    chmod +x .wteam-fake/reviewer.sh
    export WTEAM_FAKE_DIR=/app/.wteam-fake
    printf 'name = "reviewer"\ncli = "fake"\nmodel = "fake"\nprompt = "autopass"\nprofile = "review"\n' \
      >.wteam/agents/reviewer.toml
    ;;
  on) ;;
  *) echo "unsupported WTEAM_BENCH_REVIEW=$REVIEW" >&2; exit 2 ;;
esac

ROLE=$(wteam --root /app --slug "$SLUG" role planner)
PLAN_PROMPT="$ROLE

Terminal-Bench task:
$INSTRUCTION

Write .wteam/$SLUG/spec.md and 2-6 implementation tickets under .wteam/$SLUG/tickets/.
Split work across independent files or components where the task allows it; one ticket is valid for an indivisible task. Put the task's complete acceptance criteria in the spec. Do not solve the task during planning. Run wteam pickup --slug $SLUG, fix every problem it reports, and finish when the ticket graph is valid."
case $CLI in
  codex)
    codex exec --json --dangerously-bypass-approvals-and-sandbox \
      --skip-git-repo-check --model "$MODEL" --cd /app -- "$PLAN_PROMPT" \
      >/logs/agent/plan.jsonl 2>/logs/agent/plan.stderr.log
    ;;
  cursor)
    agent -p --trust --force --output-format stream-json \
      --model "$MODEL" --workspace /app "$PLAN_PROMPT" \
      >/logs/agent/plan.jsonl 2>/logs/agent/plan.stderr.log
    ;;
  opencode)
    opencode run --format json --dangerously-skip-permissions \
      -m "$OC_MODEL" -- "$PLAN_PROMPT" \
      >/logs/agent/plan.jsonl 2>/logs/agent/plan.stderr.log
    ;;
esac
ticket_count=$(find ".wteam/$SLUG/tickets" -name '*.md' -type f | wc -l)
[ "$ticket_count" -gt 0 ] || { echo "planner wrote no tickets" >&2; exit 1; }
wteam --root /app --slug "$SLUG" pickup >/logs/agent/pickup.txt
exec wteam --root /app --slug "$SLUG" run --pairs "$PAIRS" \
  --isolation none --dispatch auto --timeout "${WTEAM_RUN_TIMEOUT:-28800}" \
  --max-turns 400 --replan

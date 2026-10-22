#!/bin/sh
# wteam: a planner model decomposes the rebuild into dependent tickets, then
# worker/reviewer pairs run them with auto dispatch.
# Run outputs (plan, pickup, run, metrics, grade) go to <work dir>.logs, outside the repo.
# Env: CODEGRAPH=1 gives agents a code index; OVERLAP=allow lets tickets that share files run at once;
#   SHAPE=coupled keeps coupled classes in one ticket and adds a final integration ticket;
#   RUN_TIMEOUT caps the run in seconds (default 1800);
#   REVIEW=off swaps the reviewer for one that passes every ticket at once (an ablation).
#   sh run_wteam.sh <lib> <work dir> <planner model> <worker model> <reviewer model> <pairs>
set -u
command -v wteam >/dev/null || { echo "wteam not on PATH" >&2; exit 1; }
HERE=$(cd "$(dirname "$0")" && pwd)
LIB="$1"; DIR="$2"; PLANNER="$3"; WORKER="$4"; REVIEWER="$5"; PAIRS="$6"
sh "$HERE/prepare.sh" "$LIB" "$DIR" wteam >/dev/null
LOGS="${DIR%/}.logs"; rm -rf "$LOGS"; mkdir -p "$LOGS"; touch "$LOGS/.start"
cd "$DIR"
. "$HERE/goal.sh"
# CODEGRAPH=1: every agent gets a code index (codegraph) through wteam's project hooks:
# the worktree hook indexes each pair worktree, brief.md tells agents how to query it,
# and the profiles extend the built-ins with `Bash(codegraph *)`.
if [ "${CODEGRAPH:-0}" = 1 ]; then
  mkdir -p .wteam/hooks .wteam/profiles
  printf '#!/bin/sh\ncodegraph init "$1" >/dev/null 2>&1 || codegraph sync "$1" >/dev/null 2>&1\n' > .wteam/hooks/worktree
  printf '%s\n' 'Code index: `codegraph explore "<symbol or file names>"` prints the named symbols'"'"' current source with line numbers plus their callers and callees, in one call. Use it before Read/grep chains; treat its source output as already read.' > .wteam/brief.md
  for b in edit review plan control; do
    printf 'extends = "%s"\nallow = ["Bash(codegraph *)"]\n' "$b" > ".wteam/profiles/cg-$b.toml"
  done
  CG=cg-
  codegraph init . >/dev/null 2>&1
  mkdir -p .git/info
  printf '.codegraph/\n' >> .git/info/exclude
fi
mkdir -p .wteam/agents .wteam/roles
for spec in planner:$PLANNER:${CG:-}plan orchestrator:$WORKER:${CG:-}control worker:$WORKER:${CG:-}edit reviewer:$REVIEWER:${CG:-}review; do
  r=${spec%%:*}; rest=${spec#*:}; m=${rest%%:*}; p=${rest#*:}
  printf 'name = "%s"\ncli = "claude"\nmodel = "%s"\nprompt = "%s"\nprofile = "%s"\n' "$r" "$m" "$r" "$p" > ".wteam/agents/$r.toml"
done
if [ "${REVIEW:-on}" = off ]; then
  printf '# autopass reviewer\n\nYou are a stand-in reviewer. For the ticket named in your message, run exactly one command, `wteam verdict NN pass` with NN the ticket id, and end your turn. Read nothing else.\n' > .wteam/roles/autopass.md
  printf 'name = "reviewer"\ncli = "claude"\nmodel = "haiku"\nprompt = "autopass"\nprofile = "%sreview"\n' "${CG:-}" > .wteam/agents/reviewer.toml
fi
start=$(date +%s)
# 1. Plan: the planner role prompt plus the goal; tickets must pass `wteam pickup`.
PLAN_PROMPT="$(wteam role planner)

Goal: $GOAL
Slug: bench
Write the spec to .wteam/bench/spec.md and one ticket per file to .wteam/bench/tickets/NN-<name>.md.
Group the stubbed functions into 4-12 tickets by module, starting with a tracer bullet. The run has $PAIRS worker pairs.
Each ticket's Files line names the source files it edits. Omit Verify lines (the tests are hidden).
Finish only when \`wteam pickup --slug bench\` reports no problems."
# SHAPE=coupled: one ticket per tightly coupled class (never split a class across tickets),
# fan out only across loosely coupled modules, and end with an integration ticket.
[ "${SHAPE:-}" = coupled ] && PLAN_PROMPT="$PLAN_PROMPT
Cut tickets along coupling: a class whose methods call each other belongs to one ticket, never sliced across several; a subclass may get its own ticket blocked by its base class. Fan out only across modules that meet through a few stable signatures.
The last ticket is an integration ticket, blocked by every other ticket: it exercises the whole public API end to end (the README and docs examples) and fixes the seams between tickets. Its Files line names every source file."
[ -n "${CG:-}" ] && PLAN_PROMPT="$PLAN_PROMPT

$(cat .wteam/brief.md)"
timeout 1800 claude -p "$PLAN_PROMPT" --model "$PLANNER" --output-format json \
  --setting-sources project,local --disable-slash-commands --permission-mode dontAsk \
  --allowedTools "Read" "Edit" "Write" "Glob" "Grep" \
    "Bash(wteam *)" "Bash(ls *)" "Bash(cat *)" "Bash(python3 *)" "Bash(mkdir *)" ${CG:+"Bash(codegraph *)"} \
  > "$LOGS/plan.json" 2> "$LOGS/plan.err"
plan_end=$(date +%s)
wteam --slug bench pickup > "$LOGS/pickup.txt" 2>&1
# 2. Run.
timeout $(( ${RUN_TIMEOUT:-1800} + 600 )) wteam --slug bench run --pairs "$PAIRS" --dispatch auto --timeout "${RUN_TIMEOUT:-1800}" --max-turns 400 --replan ${OVERLAP:+--overlap "$OVERLAP"} > "$LOGS/run.txt" 2>&1
end=$(date +%s)
{
  echo "wall_seconds $((end - start))"
  echo "plan_seconds $((plan_end - start))"
  echo "tickets $(ls .wteam/bench/tickets | wc -l)"
} > "$LOGS/metrics.txt"
python3 - "$DIR" "$LOGS" >> "$LOGS/metrics.txt" <<'PY'
import glob, json, os, sys
d, logs = sys.argv[1], sys.argv[2]
def result_of(path):
    try:
        r = json.load(open(path))
    except Exception:
        return {}
    if isinstance(r, list):
        r = next((e for e in reversed(r) if e.get("type") == "result"), {})
    return r
plan = result_of(os.path.join(logs, "plan.json"))
cost = plan.get("total_cost_usd") or 0; by_role = {"planner": cost}; turns = 0; denials = len(plan.get("permission_denials") or [])
for f in glob.glob(os.path.join(d, ".wteam/bench/logs/*-[0-9]*.log")):
    role = os.path.basename(f).split(".")[0]
    for line in open(f, errors="replace"):
        try:
            e = json.loads(line)
        except ValueError:
            continue
        if e.get("type") == "result":
            c = e.get("total_cost_usd") or 0
            cost += c; turns += 1; by_role[role] = by_role.get(role, 0) + c
            denials += len(e.get("permission_denials") or [])
print("cost_usd", round(cost, 4))
print("agent_turns", turns)
print("permission_denials", denials)
cats = {}
for f in glob.glob(os.path.join(d, ".wteam/bench/logs/worker.*-[0-9]*.log")):
    for line in open(f, errors="replace"):
        try:
            e = json.loads(line)
        except ValueError:
            continue
        if e.get("type") != "assistant":
            continue
        for b in e.get("message", {}).get("content", []) or []:
            if not isinstance(b, dict) or b.get("type") != "tool_use":
                continue
            n, cmd = b.get("name"), (b.get("input") or {}).get("command", "").split()
            w = cmd[0] if cmd else ""
            k = ("explore" if n in ("Read", "Grep", "Glob") or w in ("ls", "cat", "head", "tail", "grep", "rg", "find", "sed")
                 else "index" if w == "codegraph" else "edit" if n in ("Edit", "Write", "MultiEdit") else "other")
            cats[k] = cats.get(k, 0) + 1
print("worker_tool_calls", " ".join(f"{k}={v}" for k, v in sorted(cats.items())))
for r, c in sorted(by_role.items()):
    print(f"cost_{r}", round(c, 4))
PY
grep -E "^Run stopped" "$LOGS/run.txt" >> "$LOGS/metrics.txt"
sh "$HERE/grade.sh" "$LIB" "$DIR" > "$LOGS/grade.txt"
tail -1 "$LOGS/grade.txt" >> "$LOGS/metrics.txt"
OWN="$DIR" sh "$HERE/audit.sh" "$LOGS" "$DIR/.wteam" > "$LOGS/audit.txt"
head -1 "$LOGS/audit.txt" >> "$LOGS/metrics.txt"
sh "$HERE/memorization.sh" "$LIB" "$DIR" >> "$LOGS/metrics.txt"
sh "$HERE/archive.sh" "$DIR"
cat "$LOGS/metrics.txt"

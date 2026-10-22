#!/bin/sh
# wteam: a planner model decomposes the rebuild into dependent tickets, then
# worker/reviewer pairs run them with auto dispatch.
#   sh run_wteam.sh <lib> <work dir> <planner model> <worker model> <reviewer model> <pairs>
set -u
HERE=$(cd "$(dirname "$0")" && pwd)
LIB="$1"; DIR="$2"; PLANNER="$3"; WORKER="$4"; REVIEWER="$5"; PAIRS="$6"
sh "$HERE/prepare.sh" "$LIB" "$DIR" wteam >/dev/null
cd "$DIR"
. "$HERE/goal.sh"
mkdir -p .wteam/agents
for spec in orchestrator:$WORKER:control worker:$WORKER:edit reviewer:$REVIEWER:review; do
  r=${spec%%:*}; rest=${spec#*:}; m=${rest%%:*}; p=${rest#*:}
  printf 'name = "%s"\ncli = "claude"\nmodel = "%s"\nprompt = "%s"\nprofile = "%s"\n' "$r" "$m" "$r" "$p" > ".wteam/agents/$r.toml"
done
start=$(date +%s)
# 1. Plan: the planner role prompt plus the goal; tickets must pass `wteam pickup`.
PLAN_PROMPT="$(wteam role planner)

Goal: $GOAL
Slug: bench
Write the spec to .wteam/bench/spec.md and one ticket per file to .wteam/bench/tickets/NN-<name>.md.
Group the stubbed functions into 4-12 tickets by module and dependency order, starting with a tracer bullet.
Each ticket's Files line names the source files it edits. Omit Verify lines (the tests are hidden).
Finish only when \`wteam pickup --slug bench\` reports no problems."
timeout 1800 claude -p "$PLAN_PROMPT" --model "$PLANNER" --output-format json \
  --setting-sources project,local --disable-slash-commands --permission-mode dontAsk \
  --allowedTools "Read" "Edit" "Write" "Glob" "Grep" \
    "Bash(wteam *)" "Bash(ls *)" "Bash(cat *)" "Bash(python3 *)" "Bash(mkdir *)" \
  > plan.json 2> plan.err
plan_end=$(date +%s)
wteam --slug bench pickup > pickup.txt 2>&1
# 2. Run.
timeout 7200 wteam --slug bench run --pairs "$PAIRS" --dispatch auto --timeout 1800 --max-turns 400 > run.txt 2>&1
end=$(date +%s)
{
  echo "wall_seconds $((end - start))"
  echo "plan_seconds $((plan_end - start))"
  echo "tickets $(ls .wteam/bench/tickets | wc -l)"
} > metrics.txt
python3 - "$DIR" >> metrics.txt <<'PY'
import glob, json, os, sys
d = sys.argv[1]
def result_of(path):
    try:
        r = json.load(open(path))
    except Exception:
        return {}
    if isinstance(r, list):
        r = next((e for e in reversed(r) if e.get("type") == "result"), {})
    return r
plan = result_of(os.path.join(d, "plan.json"))
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
for r, c in sorted(by_role.items()):
    print(f"cost_{r}", round(c, 4))
PY
grep -E "^Run stopped" run.txt >> metrics.txt
sh "$HERE/grade.sh" "$LIB" "$DIR" > grade.txt
tail -1 grade.txt >> metrics.txt
cat metrics.txt

#!/bin/sh
# wteam: one ticket per exercise, worker/reviewer pairs plus an orchestrator.
#   sh run_wteam.sh <polyglot checkout> <work dir> <model> <pairs>
# Every agent runs `claude -p --model <model>` with wteam's built-in profiles.
# REVIEWER_MODEL=<m> overrides the reviewer's model; DISPATCH=auto|model picks the dispatch mode.
set -u
HERE=$(cd "$(dirname "$0")" && pwd)
SRC="$1"; DIR="$2"; MODEL="$3"; PAIRS="$4"
sh "$HERE/prepare.sh" "$SRC" "$DIR" wteam >/dev/null
cd "$DIR"
mkdir -p .wteam/agents
for role in orchestrator:control worker:edit reviewer:review; do
  r=${role%%:*}; p=${role##*:}
  m="$MODEL"; [ "$r" = reviewer ] && m="${REVIEWER_MODEL:-$MODEL}"
  printf 'name = "%s"\ncli = "claude"\nmodel = "%s"\nprompt = "%s"\nprofile = "%s"\n' "$r" "$m" "$r" "$p" > ".wteam/agents/$r.toml"
done
start=$(date +%s)
timeout 5400 wteam --slug bench run --pairs "$PAIRS" --timeout 1800 --max-turns 300 ${DISPATCH:+--dispatch "$DISPATCH"} > run.txt 2>&1
end=$(date +%s)
echo "wall_seconds $((end - start))" > metrics.txt
python3 - "$DIR/.wteam/bench/logs" >> metrics.txt <<'PY'
import glob, json, os, sys
cost = 0.0; turns = 0; inp = 0; out = 0; by_role = {}; denials = 0
for f in glob.glob(os.path.join(sys.argv[1], "*-[0-9]*.log")):
    role = os.path.basename(f).split(".")[0]
    for line in open(f, errors="replace"):
        try:
            e = json.loads(line)
        except ValueError:
            continue
        if e.get("type") == "result":
            c = e.get("total_cost_usd") or 0
            u = e.get("usage", {})
            cost += c; turns += 1
            by_role[role] = by_role.get(role, 0) + c
            inp += u.get("input_tokens", 0) + u.get("cache_read_input_tokens", 0) + u.get("cache_creation_input_tokens", 0)
            out += u.get("output_tokens", 0)
            denials += len(e.get("permission_denials") or [])
print("cost_usd", round(cost, 4))
print("agent_turns", turns)
print("permission_denials", denials)
print("input_tokens", inp)
print("output_tokens", out)
for r, c in sorted(by_role.items()):
    print(f"cost_{r}", round(c, 4))
PY
grep -E "^Run stopped" run.txt >> metrics.txt
sh "$HERE/grade.sh" "$SRC" "$DIR" > grade.txt
tail -1 grade.txt >> metrics.txt
cat metrics.txt

#!/bin/sh
# Baseline: ONE headless Claude Code session rebuilds the whole library.
#   sh run_single.sh <lib> <work dir> <model>
set -u
HERE=$(cd "$(dirname "$0")" && pwd)
LIB="$1"; DIR="$2"; MODEL="$3"
sh "$HERE/prepare.sh" "$LIB" "$DIR" >/dev/null
cd "$DIR"
. "$HERE/goal.sh"
start=$(date +%s)
timeout 7200 claude -p "$GOAL Work until every stub is implemented, then report what you checked." \
  --model "$MODEL" --output-format json \
  --setting-sources project,local --disable-slash-commands \
  --permission-mode dontAsk \
  --allowedTools "Read" "Edit" "Write" "Glob" "Grep" \
    "Bash(python3 *)" "Bash(pytest *)" "Bash(ls *)" "Bash(cat *)" "Bash(mkdir *)" \
  > result.json 2> stderr.log
end=$(date +%s)
echo "wall_seconds $((end - start))" > metrics.txt
python3 - "$DIR/result.json" >> metrics.txt <<'PY'
import json, sys
try:
    r = json.load(open(sys.argv[1]))
except Exception as e:
    print("result_parse_error", e); sys.exit()
if isinstance(r, list):
    r = next((e for e in reversed(r) if e.get("type") == "result"), {})
u = r.get("usage", {})
print("cost_usd", r.get("total_cost_usd"))
print("num_turns", r.get("num_turns"))
print("permission_denials", len(r.get("permission_denials") or []))
print("input_tokens", u.get("input_tokens", 0) + u.get("cache_read_input_tokens", 0) + u.get("cache_creation_input_tokens", 0))
print("output_tokens", u.get("output_tokens"))
PY
sh "$HERE/grade.sh" "$LIB" "$DIR" > grade.txt
tail -1 grade.txt >> metrics.txt
cat metrics.txt

#!/bin/sh
# Baseline: ONE headless Claude Code session solves every exercise.
#   sh run_single.sh <polyglot checkout> <work dir> <model>   (e.g. sonnet, haiku)
# Writes <work dir>/result.json (Claude's own result event) and grade.txt.
set -u
HERE=$(cd "$(dirname "$0")" && pwd)
SRC="$1"; DIR="$2"; MODEL="$3"
sh "$HERE/prepare.sh" "$SRC" "$DIR" >/dev/null
cd "$DIR"
if [ "${HIDDEN:-0}" = 1 ]; then
PROMPT='Solve every exercise in this repository. Each top-level directory is one exercise with INSTRUCTIONS.md (the statement) and a stub <name>.py. Implement each stub exactly as its INSTRUCTIONS.md specifies, including every edge case and error it describes. A hidden test suite grades the result, so write and run your own checks with python3. Work through ALL directories before you finish.'
else
PROMPT='Solve every exercise in this repository. Each top-level directory is one exercise with INSTRUCTIONS.md (the statement), a stub <name>.py and <name>_test.py. Implement each stub so its tests pass; check with `python3 -m pytest -q <dir>`. Never edit test files. Work through ALL directories before you finish, then report how many pass.'
fi
start=$(date +%s)
# Same isolation and permission shape as wteam workers: project settings only,
# no skills, edits and python/pytest allowed, everything else denied.
timeout 5400 claude -p "$PROMPT" --model "$MODEL" --output-format json \
  --setting-sources project,local --disable-slash-commands \
  --permission-mode dontAsk \
  --allowedTools "Read" "Edit" "Write" "Glob" "Grep" \
    "Bash(python3 *)" "Bash(pytest *)" "Bash(ls *)" "Bash(cat *)" \
  > result.json 2> stderr.log
end=$(date +%s)
echo "wall_seconds $((end - start))" > metrics.txt
python3 - "$DIR/result.json" >> metrics.txt <<'PY'
import json, sys
try:
    r = json.load(open(sys.argv[1]))
except Exception as e:
    print("result_parse_error", e); sys.exit()
if isinstance(r, list):  # some versions emit every event; keep the final result event
    r = next((e for e in reversed(r) if e.get("type") == "result"), {})
u = r.get("usage", {})
print("cost_usd", r.get("total_cost_usd"))
print("num_turns", r.get("num_turns"))
print("permission_denials", len(r.get("permission_denials") or []))
print("input_tokens", u.get("input_tokens", 0) + u.get("cache_read_input_tokens", 0) + u.get("cache_creation_input_tokens", 0))
print("output_tokens", u.get("output_tokens"))
PY
sh "$HERE/grade.sh" "$SRC" "$DIR" > grade.txt
tail -1 grade.txt >> metrics.txt
cat metrics.txt

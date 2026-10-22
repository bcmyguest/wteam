#!/bin/sh
# Baseline: ONE headless Claude Code session rebuilds the whole library.
# Run outputs go to <work dir>.logs, outside the repo.
#   sh run_single.sh <lib> <work dir> <model>
# Env: PLAN=<dir> hands the session a saved plan (spec.md + tickets) to work through in order.
set -u
HERE=$(cd "$(dirname "$0")" && pwd)
LIB="$1"; DIR="$2"; MODEL="$3"
sh "$HERE/prepare.sh" "$LIB" "$DIR" >/dev/null
LOGS="${DIR%/}.logs"; rm -rf "$LOGS"; mkdir -p "$LOGS"; touch "$LOGS/.start"
cd "$DIR"
. "$HERE/goal.sh"
PROMPT="$GOAL Work until every stub is implemented, then report what you checked."
if [ -n "${PLAN:-}" ]; then
  PROMPT="$PROMPT

Follow this plan: work through the tickets one at a time, each only after its Blocked-by tickets.
$(for f in "$PLAN"/spec.md "$PLAN"/[0-9]*.md; do printf '\n----- %s -----\n' "$(basename "$f")"; cat "$f"; done)"
fi
start=$(date +%s)
timeout 7200 claude -p "$PROMPT" \
  --model "$MODEL" --output-format json \
  --setting-sources project,local --disable-slash-commands \
  --permission-mode dontAsk \
  --allowedTools "Read" "Edit" "Write" "Glob" "Grep" \
    "Bash(python3 *)" "Bash(pytest *)" "Bash(ls *)" "Bash(cat *)" "Bash(mkdir *)" \
  > "$LOGS/result.json" 2> "$LOGS/stderr.log"
end=$(date +%s)
echo "wall_seconds $((end - start))" > "$LOGS/metrics.txt"
python3 - "$LOGS/result.json" >> "$LOGS/metrics.txt" <<'PY'
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
sh "$HERE/grade.sh" "$LIB" "$DIR" > "$LOGS/grade.txt"
tail -1 "$LOGS/grade.txt" >> "$LOGS/metrics.txt"
# Claude Code keeps the full transcript per project dir; keep this run's copy as evidence.
T="$HOME/.claude/projects/$(printf '%s' "$DIR" | sed 's#[/.]#-#g')"
mkdir -p "$LOGS/transcripts"
[ -d "$T" ] && find "$T" -maxdepth 1 -name '*.jsonl' -newer "$LOGS/.start" -exec cp {} "$LOGS/transcripts/" \;
OWN="$DIR" sh "$HERE/audit.sh" "$LOGS" > "$LOGS/audit.txt"
head -1 "$LOGS/audit.txt" >> "$LOGS/metrics.txt"
sh "$HERE/memorization.sh" "$LIB" "$DIR" >> "$LOGS/metrics.txt"
mv "$LOGS/transcripts" "$DIR/.transcripts"   # transcripts hold code: archived, not left in /tmp
sh "$HERE/archive.sh" "$DIR"
cat "$LOGS/metrics.txt"

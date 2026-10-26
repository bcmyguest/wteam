#!/bin/sh
# Flags tool calls that peek at hidden grader material or other benchmark runs.
# OWN (env) is this run's repo dir; paths under it (and <dir>.logs) are allowed.
#   OWN=<repo dir> sh audit.sh <log dirs or files>...   → "leak_suspects N"
set -u
find "$@" -type f \( -name '*.json' -o -name '*.log' -o -name '*.jsonl' \) 2>/dev/null | python3 -c '
import json, re, sys
import os
pat = re.compile(
    r"e8d709d|ba759a4|apply_adaptations|bench/brownfield/grader|"
    r"hidden_gate\.sh|git (log|show|archive)|commit0-cache|commit0-ref|"
    r"wteam-bench/(runs|tasks)|pip3? (download|install|show)|find / "
)
own = os.environ.get("OWN", "\0")
other_run = re.compile(r"/tmp/(?!claude-)[\w.-]+[\w./-]*")

def flagged(text):
    return pat.search(text) or any(
        not m.group(0).startswith(own) and not m.group(0).startswith(own + ".logs")
        for m in other_run.finditer(text)
    )

hits = set()

def walk(o):
    if isinstance(o, dict):
        if o.get("type") == "tool_use" and flagged(json.dumps(o.get("input"))):
            hits.add(json.dumps(o["input"])[:200])
        for p in o.get("permission_denials") or []:
            if flagged(json.dumps(p.get("tool_input"))):
                hits.add("DENIED " + json.dumps(p["tool_input"])[:200])
        for v in o.values():
            walk(v)
    elif isinstance(o, list):
        for v in o:
            walk(v)

for f in sys.stdin.read().split():
    for line in open(f, errors="replace"):
        try:
            walk(json.loads(line))
        except ValueError:
            try:
                walk(json.load(open(f)))
                break
            except ValueError:
                break
print("leak_suspects", len(hits))
for h in sorted(hits):
    print("  " + h)
'

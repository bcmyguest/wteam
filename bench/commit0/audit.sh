#!/bin/sh
# Flags agent tool calls that reach for the hidden tests or the original source: the bench
# cache, the reference, the venv's installed packages, package downloads. Tool-installed
# linters (pip install pycodestyle) also show; judge the hits by hand.
#   sh audit.sh <file or dir of agent JSON logs>...   → "leak_suspects N" and the hits
set -u
find "$@" -type f \( -name '*.json' -o -name '*.log' \) | python3 -c '
import json, re, sys
pat = re.compile(r"commit0-cache|commit0-ref|commit0-venvs/[^/ ]*/lib|site-packages|pip3? (download|install)|pypi|github\.com/scott-griffiths")
hits = set()
def walk(o):
    if isinstance(o, dict):
        if o.get("type") == "tool_use" and pat.search(json.dumps(o.get("input"))):
            hits.add(json.dumps(o["input"])[:200])
        for p in o.get("permission_denials") or []:
            if pat.search(json.dumps(p.get("tool_input"))):
                hits.add("DENIED " + json.dumps(p["tool_input"])[:200])
        for v in o.values(): walk(v)
    elif isinstance(o, list):
        for v in o: walk(v)
for f in sys.stdin.read().split():
    for line in open(f, errors="replace"):
        try: walk(json.loads(line))
        except ValueError:
            try: walk(json.load(open(f))); break
            except ValueError: break
print("leak_suspects", len(hits))
for h in sorted(hits): print("  " + h)
'

#!/bin/sh
# Flags agent tool calls that reach for the hidden tests or the original source: the bench
# cache, the reference, the venv's installed packages, package downloads, another run's repo.
# Tool-installed linters (pip install pycodestyle) also show; judge the hits by hand.
# OWN (env) is this run's repo dir: paths inside it are never flagged as another run's.
#   OWN=<repo dir> sh audit.sh <file or dir of agent logs/transcripts>...   → "leak_suspects N"
set -u
find "$@" -type f \( -name '*.json' -o -name '*.log' -o -name '*.jsonl' \) | python3 -c '
import json, re, sys
import os
pat = re.compile(r"commit0-cache|commit0-ref|commit0-venvs/[^/ ]*/lib|site-packages|pip3? (download|install|show)|find / |pypi|github\.com/scott-griffiths|wteam-bench/runs")
own = os.environ.get("OWN", "\0")
other_run = re.compile(r"/tmp/[\w.-]*(bitstring|tinydb|voluptuous|marshmallow|imapclient)[\w./-]*")
def flagged(text):
    return pat.search(text) or any(not m.group(0).startswith(own) and not m.group(0).startswith(own + ".logs") for m in other_run.finditer(text))
hits = set()
def walk(o):
    if isinstance(o, dict):
        if o.get("type") == "tool_use" and flagged(json.dumps(o.get("input"))):
            hits.add(json.dumps(o["input"])[:200])
        for p in o.get("permission_denials") or []:
            if flagged(json.dumps(p.get("tool_input"))):
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

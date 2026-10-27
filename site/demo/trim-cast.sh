#!/bin/sh
# Trim asciicast: end on git log, drop tmux "[exited]" and trailing dashboard noise.
set -eu
in=$1
out=${2:-$in}
python3 - "$in" "$out" <<'PY'
import sys

src, dst = sys.argv[1], sys.argv[2]
with open(src, encoding="utf-8") as f:
    lines = f.readlines()
if not lines:
    sys.exit(1)
header, events = lines[0], [ln for ln in lines[1:] if ln.startswith("[")]
kept: list[str] = []
stop = False
for ev in events:
    if stop:
        break
    if "[exited]" in ev:
        break
    kept.append(ev)
    # Last meaningful step in the run pane (git log); skip agent-pane refreshes after.
    if "feat: CLI glue" in ev and "chore: init" in ev:
        stop = True
with open(dst, "w", encoding="utf-8") as f:
    f.writelines([header, *kept])
PY
printf '%s\n' "$out"

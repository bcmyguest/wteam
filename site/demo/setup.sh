#!/bin/sh
# Builds the repo the demo GIF records: four tickets (01 → 02,03 → 04) and
# scripted stand-in agents (cli = "fake") so the recording is fast and repeatable.
set -eu
D="${1:-/tmp/wteam-demo}"
SCRIPT_DIR=$(CDPATH= cd -- "$(dirname "$0")" && pwd)
rm -rf "$D" && mkdir -p "$D" && cd "$D"
git init -q -b main
git config user.email demo@example.com && git config user.name demo
echo "# demo" > README.md && git add -A && git commit -qm "chore: init"
wteam --slug demo init >/dev/null
cp "$SCRIPT_DIR/dashboard.sh" "$SCRIPT_DIR/tmux-start.sh" "$D/"
chmod +x "$D/dashboard.sh" "$D/tmux-start.sh"
T=.wteam/demo/tickets
ticket() { # id title blockers file
  printf '# %s: %s\n\n**What to build:** %s.\n\n**Blocked by:** %s\n\n**Status:** ready-for-agent\n\n**Files:** %s\n\n- [ ] done\n' \
    "$1" "$2" "$2" "$3" "$4" > "$T/$1-$(echo "$2" | tr ' A-Z' '-a-z').md"
}
ticket 01 "Tracer bullet" "None (can start immediately)" src/core.py
ticket 02 "Parser" "01" src/parser.py
ticket 03 "Renderer" "01" src/render.py
ticket 04 "CLI glue" "02, 03" src/greet.py
mkdir -p .wteam/agents .wteam-fake
for r in orchestrator worker reviewer; do
  printf 'name = "%s"\ncli = "fake"\nprompt = "%s"\nprofile = "edit"\n' $r $r > .wteam/agents/$r.toml
done
cat > .wteam-fake/orchestrator.sh <<'S'
wteam ready | awk '/^[0-9]/{print $1}' | while read t; do wteam dispatch "$t" >/dev/null 2>&1 || true; done
S
cat > .wteam-fake/worker.sh <<'S'
T=$(wteam status | awk -v a="$WTEAM_AGENT" '$NF==a && /\[doing\]/ {print $1; exit}')
F=$(sed -n 's/^\*\*Files:\*\* //p' "$WTEAM_ROOT"/.wteam/demo/tickets/$T-*.md)
sleep 2
mkdir -p "$(dirname "$F")"
case "$T" in
01)
  cat > "$F" <<'PY'
"""Shared greet constants."""
VERSION = "0.1.0"
DEFAULT_NAME = "world"
PY
  ;;
02)
  cat > "$F" <<'PY'
def parse_name(argv):
    return argv[1] if len(argv) > 1 else "world"
PY
  ;;
03)
  cat > "$F" <<'PY'
def render(name: str) -> str:
    return f"hello, {name}"
PY
  ;;
04)
  cat > "$F" <<'PY'
#!/usr/bin/env python3
from parser import parse_name
from render import render
import sys

def main() -> None:
    print(render(parse_name(sys.argv)))

if __name__ == "__main__":
    main()
PY
  chmod +x "$F"
  ;;
esac
wteam submit "$T" --summary "implemented $T"
S
cat > .wteam-fake/reviewer.sh <<'S'
T=$(wteam status | awk -v a="$WTEAM_AGENT" '$NF==a && /\[review\]/ {print $1; exit}')
sleep 1
wteam verdict "$T" pass
S

#!/bin/sh
# Builds the repo the demo GIF records: four tickets (01 → 02,03 → 04) and
# scripted stand-in agents (cli = "fake") so the recording is fast and repeatable.
set -eu
D="${1:-/tmp/wteam-demo}"
rm -rf "$D" && mkdir -p "$D" && cd "$D"
git init -q -b main
git config user.email demo@example.com && git config user.name demo
echo "# demo" > README.md && git add -A && git commit -qm "chore: init"
wteam --slug demo init >/dev/null
T=.wteam/demo/tickets
ticket() { # id title blockers file
  printf '# %s: %s\n\n**What to build:** %s.\n\n**Blocked by:** %s\n\n**Status:** ready-for-agent\n\n**Files:** %s\n\n- [ ] done\n' \
    "$1" "$2" "$2" "$3" "$4" > "$T/$1-$(echo "$2" | tr ' A-Z' '-a-z').md"
}
ticket 01 "Tracer bullet" "None (can start immediately)" src/core.txt
ticket 02 "Parser" "01" src/parser.txt
ticket 03 "Renderer" "01" src/render.txt
ticket 04 "CLI glue" "02, 03" src/cli.txt
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
sleep 1; mkdir -p src; echo "ticket $T" > "$F"
wteam submit "$T" --summary "implemented $T"
S
cat > .wteam-fake/reviewer.sh <<'S'
T=$(wteam status | awk -v a="$WTEAM_AGENT" '$NF==a && /\[review\]/ {print $1; exit}')
sleep 1; wteam verdict "$T" pass
S

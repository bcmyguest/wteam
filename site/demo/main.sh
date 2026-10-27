#!/bin/sh
# Visible demo steps (run pane inside tmux during recording).
set -eu
. "$(dirname "$0")/env.sh"
cd "$DEMO_ROOT"

sleep 1
printf '# greet CLI: wteam tickets add, then run the team\n\n'
sleep 1
sh seed-tickets.sh
sleep 2
wteam --slug demo tickets list
sleep 2
wteam --slug demo run --pairs 2
sleep 1
sed -n '1,12p' src/greet.py
sleep 2
git --no-pager log --oneline -5
sleep 2
tmux kill-session -t wteam-demo

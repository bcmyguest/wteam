#!/bin/sh
# Top pane: wteam run. Bottom pane: live agent dashboard (demo GIF).
set -eu
cd /tmp/wteam-demo
export PATH="/opt/wteam:${PATH}"
export WTEAM_FAKE_DIR=/tmp/wteam-demo/.wteam-fake
export WTEAM_DEMO_ROOT=/tmp/wteam-demo
export WTEAM_DEMO_PATH="/opt/wteam:${PATH}"

tmux split-window -d -v -p 30 -- bash
tmux send-keys -t 1 "export PATH=/opt/wteam:\$PATH WTEAM_DEMO_ROOT=/tmp/wteam-demo WTEAM_DEMO_PATH=/opt/wteam:\$PATH" Enter
tmux send-keys -t 1 "while true; do sh dashboard.sh; sleep 0.5; done" Enter
tmux select-pane -t 0
tmux set-option pane-border-status top
tmux set-window-option pane-border-format ' #{pane_index} #{pane_title}'
tmux select-pane -t 0 -T run
tmux select-pane -t 1 -T agents

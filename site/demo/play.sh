#!/bin/sh
# Full demo for terminal recording (asciinema): run + tickets + agents panes.
set -eu
. "$(dirname "$0")/env.sh"

if [ ! -x "$WTEAM_BIN/wteam" ]; then
  (cd "$REPO" && cargo build --release -q)
fi

sh "$SCRIPT_DIR/setup.sh" >/dev/null
cd "$DEMO_ROOT"

pane_env="export PATH=\"$WTEAM_DEMO_PATH\" WTEAM_DEMO_ROOT=\"$DEMO_ROOT\" WTEAM_DEMO_SLUG=demo"

tmux kill-server 2>/dev/null || true
tmux new-session -d -s wteam-demo -c "$DEMO_ROOT"
tmux split-window -t wteam-demo -v -p 34
tmux split-window -t wteam-demo:0.1 -h -p 50
tmux send-keys -t wteam-demo:0.1 "$pane_env; sh dashboard.sh tickets" C-m
tmux send-keys -t wteam-demo:0.2 "$pane_env; sh dashboard.sh agents" C-m
tmux set-option -t wteam-demo pane-border-status top
tmux set-window-option -t wteam-demo pane-border-format ' #{pane_title}'
tmux select-pane -t wteam-demo:0.0 -T run
tmux select-pane -t wteam-demo:0.1 -T tickets
tmux select-pane -t wteam-demo:0.2 -T agents
tmux send-keys -t wteam-demo:0.0 "sh \"$SCRIPT_DIR/main.sh\"" C-m
exec tmux attach -t wteam-demo

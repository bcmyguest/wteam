#!/bin/sh
# Full demo for terminal recording (asciinema): tmux split + main.sh in the run pane.
set -eu
. "$(dirname "$0")/env.sh"

if [ ! -x "$WTEAM_BIN/wteam" ]; then
  (cd "$REPO" && cargo build --release -q)
fi

sh "$SCRIPT_DIR/setup.sh" >/dev/null
cd "$DEMO_ROOT"

tmux kill-server 2>/dev/null || true
tmux new-session -d -s wteam-demo -c "$DEMO_ROOT"
tmux split-window -t wteam-demo -v -p 30
tmux send-keys -t wteam-demo:0.1 \
  "export PATH=\"$WTEAM_DEMO_PATH\" WTEAM_DEMO_ROOT=\"$DEMO_ROOT\" WTEAM_DEMO_SLUG=demo; sh dashboard.sh" C-m
tmux set-option -t wteam-demo pane-border-status top
tmux set-window-option -t wteam-demo pane-border-format ' #{pane_index} #{pane_title}'
tmux select-pane -t wteam-demo:0.0 -T run
tmux select-pane -t wteam-demo:0.1 -T agents
tmux send-keys -t wteam-demo:0.0 "sh \"$SCRIPT_DIR/main.sh\"" C-m
exec tmux attach -t wteam-demo

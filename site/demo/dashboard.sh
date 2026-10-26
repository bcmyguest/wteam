#!/bin/sh
# Compact live dashboard for the demo GIF (refreshed from tmux split pane).
set -eu
ROOT="${WTEAM_DEMO_ROOT:-/tmp/wteam-demo}"
SLUG="${WTEAM_DEMO_SLUG:-demo}"

cd "$ROOT"
export PATH="${WTEAM_DEMO_PATH:-$PATH}"

printf '\033[H\033[J'
printf '\033[1;36m wteam\033[0m \033[2m· agents\033[0m\n'
printf '\033[2m────────────────────────────────────────\033[0m\n'

wteam --slug "$SLUG" status 2>/dev/null | awk '
/^[0-9][0-9]  / {
  id = $1
  st = "todo"
  if (match($0, /\[[a-z_]+\]/)) {
    st = substr($0, RSTART + 1, RLENGTH - 2)
  }
  title = $0
  sub(/^[^ ]+  /, "", title)
  agent = ""
  if (match($0, /  [a-z]+\.[0-9]+ *$/)) {
    agent = substr($0, RSTART + 2, RLENGTH - 2)
    sub(/  [a-z]+\.[0-9]+ *$/, "", title)
  }
  sub(/  \[[^]]+\].*/, "", title)
  icon = "○"
  if (st == "doing") icon = "◉"
  else if (st == "review") icon = "◎"
  else if (st == "done") icon = "✓"
  else if (st == "blocked") icon = "✕"
  if (agent != "") {
    printf " %s  %-3s  %-12s  [%s]  %s\n", icon, id, title, st, agent
  } else {
    printf " %s  %-3s  %-12s  [%s]\n", icon, id, title, st
  }
  next
}
/^agents:/ { agents = 1; next }
agents && /^  / {
  name = $1; run = $2;
  mark = (run == "busy") ? "▸" : " ";
  printf " %s \033[1m%-14s\033[0m %s\n", mark, name, run;
  next
}
'

printf '\033[2m────────────────────────────────────────\033[0m\n'
printf ' \033[2m%s\033[0m\n' "$(date +%H:%M:%S)"

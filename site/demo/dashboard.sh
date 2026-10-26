#!/bin/sh
# Compact live dashboard for the demo GIF (one pane, in-place refresh).
set -eu
ROOT="${WTEAM_DEMO_ROOT:-/tmp/wteam-demo}"
SLUG="${WTEAM_DEMO_SLUG:-demo}"

cd "$ROOT"
export PATH="${WTEAM_DEMO_PATH:-$PATH}"

# Print one line and clear any leftover characters from the previous frame.
dash_line() {
  # shellcheck disable=SC2059
  printf '%b\033[K\n' "$1"
}

first=1
while true; do
  if [ "$first" -eq 1 ]; then
    printf '\033[2J\033[H'
    first=0
  else
    printf '\033[H'
  fi

  dash_line '\033[1;36m wteam\033[0m \033[2m· agents\033[0m'
  dash_line '\033[2m────────────────────────────────────────\033[0m'

  wteam --slug "$SLUG" tickets list 2>/dev/null | awk '
  /^[0-9][0-9]  / {
    id = $1
    st = "todo"
    if (match($0, /\[[a-z_]+\]/)) {
      st = substr($0, RSTART + 1, RLENGTH - 2)
    }
    title = $0
    sub(/^[^ ]+  /, "", title)
    sub(/  \[[^]]+\].*/, "", title)
    agent = ""
    if (match($0, /assignee=[^ ]+/)) {
      agent = substr($0, RSTART + 10, RLENGTH - 10)
    }
    icon = "○"
    if (st == "doing") icon = "◉"
    else if (st == "review") icon = "◎"
    else if (st == "done") icon = "✓"
    else if (st == "blocked") icon = "✕"
    if (agent != "") {
      printf " %s  %-3s  %-12s  [%s]  %s", icon, id, title, st, agent
    } else {
      printf " %s  %-3s  %-12s  [%s]", icon, id, title, st
    }
    print ""
  }
  ' | while IFS= read -r row; do
    dash_line "$row"
  done

  wteam --slug "$SLUG" status 2>/dev/null | awk '
  /^agents:/ { agents = 1; next }
  agents && /^  / {
    name = $1; run = $2;
    mark = (run == "busy") ? "▸" : " "
    printf " %s \033[1m%-14s\033[0m %s", mark, name, run
    print ""
  }
  ' | while IFS= read -r row; do
    dash_line "$row"
  done

  dash_line '\033[2m────────────────────────────────────────\033[0m'
  dash_line ' \033[2mrefreshing…\033[0m'

  sleep 0.8
done

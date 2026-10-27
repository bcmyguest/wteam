#!/bin/sh
# Drop tmux "[exited]" and any trailing events from an asciicast (stdout: trimmed path).
set -eu
in=$1
out=${2:-$in}
tmp=$(mktemp)
head -n 1 "$in" > "$tmp"
awk '
  /^\[/ {
    if ($0 ~ /\[exited\]/) exit
    print
  }
' "$in" >> "$tmp"
mv "$tmp" "$out"
printf '%s\n' "$out"

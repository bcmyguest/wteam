#!/bin/sh
# Render site/public/demo.gif via asciinema + agg (see demo/play.sh).
set -eu
SITE=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
REPO=$(CDPATH= cd -- "$SITE/.." && pwd)
WTEAM_BIN="${WTEAM_BIN:-$REPO/target/release}"
export WTEAM_BIN

if [ ! -x "$WTEAM_BIN/wteam" ]; then
  (cd "$REPO" && cargo build --release -q)
fi

if command -v asciinema >/dev/null && command -v agg >/dev/null && command -v tmux >/dev/null; then
  cd "$SITE"
  export TERM="${TERM:-xterm-256color}"
  cast="$SITE/demo/.cache/demo.cast"
  mkdir -p "$(dirname "$cast")"
  asciinema rec -y --cols 120 --rows 40 -c "sh demo/play.sh" -t "wteam demo" --overwrite "$cast"
  trimmed="$SITE/demo/.cache/demo.trimmed.cast"
  sh "$SITE/demo/trim-cast.sh" "$cast" "$trimmed" >/dev/null
  agg --font-size 14 --last-frame-duration 2 "$trimmed" public/demo.gif
  exit 0
fi

echo "asciinema, agg, and tmux not all on PATH; recording in Docker…" >&2
docker run --rm \
  -v "$SITE":/site \
  -v "$REPO":/repo \
  -e AGG_VERSION="${AGG_VERSION:-1.9.0}" \
  rust:1-bookworm \
  bash /site/demo/render-docker.sh

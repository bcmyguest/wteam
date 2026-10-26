#!/bin/sh
# Render site/public/demo.gif (needs Docker, network for tmux in the VHS image).
set -eu
SITE=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
REPO=$(CDPATH= cd -- "$SITE/.." && pwd)
WTEAM_BIN="${WTEAM_BIN:-$REPO/target/release}"
if [ ! -x "$WTEAM_BIN/wteam" ]; then
  (cd "$REPO" && cargo build --release -q)
fi
docker run --rm \
  -v "$SITE":/vhs \
  -v "$WTEAM_BIN":/opt/wteam \
  -w /vhs \
  --entrypoint bash \
  ghcr.io/charmbracelet/vhs \
  -c 'apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq git tmux >/dev/null && vhs demo.tape'

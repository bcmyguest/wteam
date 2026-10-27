#!/bin/bash
# Runs in Docker (see render.sh): build wteam, record with asciinema, convert with agg.
set -euo pipefail
SITE=/site
REPO=/repo
export DEBIAN_FRONTEND=noninteractive

apt-get update -qq
apt-get install -y -qq git tmux asciinema curl ca-certificates fonts-dejavu-core >/dev/null

AGG_VERSION="${AGG_VERSION:-1.9.0}"
AGG_ARCH="${AGG_ARCH:-x86_64-unknown-linux-musl}"
AGG_URL="https://github.com/asciinema/agg/releases/download/v${AGG_VERSION}/agg-${AGG_ARCH}"
tmpdir=$(mktemp -d)
curl -fsSL "$AGG_URL" -o "$tmpdir/agg"
install -m 755 "$tmpdir/agg" /usr/local/bin/agg
rm -rf "$tmpdir"

cd "$REPO"
cargo build --release -q
export WTEAM_BIN="$REPO/target/release"
export PATH="$WTEAM_BIN:$PATH"

cd "$SITE"
export TERM=xterm-256color

cast="$SITE/demo/.cache/demo.cast"
mkdir -p "$(dirname "$cast")"
asciinema rec -y --cols 120 --rows 36 -c "sh demo/play.sh" -t "wteam demo" --overwrite "$cast"
trimmed="$SITE/demo/.cache/demo.trimmed.cast"
sh "$SITE/demo/trim-cast.sh" "$cast" "$trimmed" >/dev/null
agg --font-size 14 --last-frame-duration 1 "$trimmed" public/demo.gif

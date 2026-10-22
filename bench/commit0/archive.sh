#!/bin/sh
# Moves a graded run's repo out of reach of later runs: tars it under ~/.cache/wteam-bench/runs
# (agents cannot run tar) and deletes the directory. Run logs (<dir>.logs) stay in place.
#   sh archive.sh <repo dir>
set -eu
DIR="${1%/}"
OUT="${BENCH_ARCHIVE:-$HOME/.cache/wteam-bench/runs}"
mkdir -p "$OUT"
[ -d "$DIR" ] || exit 0
tar czf "$OUT/$(basename "$DIR")-$(date +%Y%m%d-%H%M%S).tar.gz" -C "$(dirname "$DIR")" "$(basename "$DIR")"
rm -rf "$DIR"

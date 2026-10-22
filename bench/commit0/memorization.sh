#!/bin/sh
# Contamination check: how much of the rebuilt library is upstream's code, verbatim.
# Downloads the upstream release into a temp dir, compares every stubbed function body
# (memorization.py), then deletes the download.   sh memorization.sh <lib> <repo dir>
set -u
LIB="$1"; REPO="$2"
CACHE="${BENCH_CACHE:-/tmp/commit0-cache}"
case "$LIB" in
  tinydb) SPEC=tinydb==4.8.0; PKG=tinydb; SRC=tinydb ;;
  voluptuous) SPEC=voluptuous==0.15.2; PKG=voluptuous; SRC=voluptuous ;;
  bitstring) SPEC=bitstring==4.2.3; PKG=bitstring; SRC=bitstring ;;
  marshmallow) SPEC=marshmallow==3.21.3; PKG=marshmallow; SRC=src/marshmallow ;;
  imapclient) SPEC=imapclient==3.0.1; PKG=imapclient; SRC=imapclient ;;
  *) echo "memorization n/a (no upstream version pinned for $LIB)"; exit 0 ;;
esac
tmp=$(mktemp -d)
python3 -m pip download -q "$SPEC" --no-deps -d "$tmp" >/dev/null 2>&1 && (cd "$tmp" && python3 -m zipfile -e ./*.whl up >/dev/null 2>&1)
if [ -d "$tmp/up/$PKG" ]; then
  python3 -I "$(dirname "$0")/memorization.py" "$CACHE/$LIB/$SRC" "$tmp/up/$PKG" "$REPO/$SRC" | sed 's/^[^ ]* /memorization /'
else
  echo "memorization n/a (download failed)"
fi
rm -rf "$tmp"

#!/bin/sh
# Install the wteam binary from a GitHub release on macOS or Linux (WSL included).
#
#   curl -fsSL https://raw.githubusercontent.com/bcmyguest/wteam/main/install.sh | sh
#   sh install.sh                      # from a checkout
#
# Downloads go through an authenticated `gh` when available, else plain curl.
#
# Environment:
#   WTEAM_VERSION       release tag to install (default: latest), e.g. v0.1.0
#   WTEAM_INSTALL_DIR   install directory (default: ~/.local/bin)
#   WTEAM_REPO          owner/repo (default: bcmyguest/wteam)
#   WTEAM_DOWNLOAD_BASE base URL holding the release assets; forces curl (tests, mirrors)
#   WTEAM_OS, WTEAM_ARCH  override `uname -s` / `uname -m` (tests)
set -eu

repo="${WTEAM_REPO:-bcmyguest/wteam}"
version="${WTEAM_VERSION:-latest}"
install_dir="${WTEAM_INSTALL_DIR:-$HOME/.local/bin}"

say() { printf 'wteam-install: %s\n' "$*"; }
die() { printf 'wteam-install: error: %s\n' "$*" >&2; exit 1; }

os="${WTEAM_OS:-$(uname -s)}"
arch="${WTEAM_ARCH:-$(uname -m)}"
case "$os/$arch" in
  Linux/x86_64 | Linux/amd64) target=x86_64-unknown-linux-musl ;;
  Linux/aarch64 | Linux/arm64) target=aarch64-unknown-linux-musl ;;
  Darwin/arm64 | Darwin/aarch64) target=aarch64-apple-darwin ;;
  Darwin/x86_64) target=x86_64-apple-darwin ;;
  *) die "no prebuilt binary for $os/$arch; build from source: cargo install --git https://github.com/$repo --locked" ;;
esac
asset="wteam-$target"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT INT TERM

fetch_curl() {
  if [ -n "${WTEAM_DOWNLOAD_BASE:-}" ]; then
    base="$WTEAM_DOWNLOAD_BASE"
  elif [ "$version" = latest ]; then
    base="https://github.com/$repo/releases/latest/download"
  else
    base="https://github.com/$repo/releases/download/$version"
  fi
  for f in "$asset" checksums.txt; do
    curl -fsSL "$base/$f" -o "$tmp/$f" || die "download failed: $base/$f"
  done
}

fetch_gh() {
  tag=""
  [ "$version" = latest ] || tag="$version"
  # shellcheck disable=SC2086 # empty $tag must vanish
  gh release download $tag -R "$repo" -p "$asset" -p checksums.txt -D "$tmp" \
    || die "gh release download failed for $repo ${tag:-latest}"
}

if [ -z "${WTEAM_DOWNLOAD_BASE:-}" ] && command -v gh >/dev/null 2>&1 && gh auth status >/dev/null 2>&1; then
  say "downloading $asset ($version) with gh"
  fetch_gh
else
  command -v curl >/dev/null 2>&1 || die "need curl (or an authenticated gh)"
  say "downloading $asset ($version) with curl"
  fetch_curl
fi

expected="$(awk -v f="$asset" '$2 == f || $2 == "*"f { print $1 }' "$tmp/checksums.txt")"
[ -n "$expected" ] || die "checksums.txt has no entry for $asset"
if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "$tmp/$asset" | awk '{print $1}')"
elif command -v shasum >/dev/null 2>&1; then
  actual="$(shasum -a 256 "$tmp/$asset" | awk '{print $1}')"
else
  die "need sha256sum or shasum to verify the download"
fi
[ "$actual" = "$expected" ] || die "checksum mismatch for $asset (expected $expected, got $actual)"

mkdir -p "$install_dir"
chmod +x "$tmp/$asset"
mv "$tmp/$asset" "$install_dir/wteam"
say "installed $install_dir/wteam"

case ":$PATH:" in
  *":$install_dir:"*) ;;
  *) say "add $install_dir to PATH, e.g. echo 'export PATH=\"$install_dir:\$PATH\"' >> ~/.profile" ;;
esac
say "agent skill: npx skills add $repo"

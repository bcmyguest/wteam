# wteam

[![CI](https://github.com/bcmyguest/wteam/actions/workflows/ci.yml/badge.svg)](https://github.com/bcmyguest/wteam/actions/workflows/ci.yml)
<!-- DELETE the crates.io badge if not publishing to crates.io -->
[![crates.io](https://img.shields.io/crates/v/wteam.svg)](https://crates.io/crates/wteam)

Portable worker/reviewer teams for coding agents (Claude Code, Codex, opencode, Cursor)

## Install

<!-- KEEP the section(s) matching your decisions; delete the rest. -->

<!-- IF publishing to crates.io -->
```bash
cargo install wteam --locked
```

<!-- IF CLI with binary releases -->
Or grab a static binary from the
[latest release](https://github.com/bcmyguest/wteam/releases/latest) —
`x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`,
`aarch64-apple-darwin`, or `x86_64-apple-darwin`:

```bash
case "$(uname -sm)" in
  "Linux x86_64")   target=x86_64-unknown-linux-musl ;;
  "Linux aarch64")  target=aarch64-unknown-linux-musl ;;
  "Darwin arm64")   target=aarch64-apple-darwin ;;
  "Darwin x86_64")  target=x86_64-apple-darwin ;;
esac
curl -fsSL -o ~/.local/bin/wteam \
  "https://github.com/bcmyguest/wteam/releases/latest/download/wteam-$target"
chmod +x ~/.local/bin/wteam
```

Or build from source:

```bash
git clone https://github.com/bcmyguest/wteam
cd wteam
cargo build --release   # binary at target/release/wteam
```

## Usage

<!-- Replace with real usage once the hello-world mock is gone. -->

```bash
wteam Ferris
# Hello, Ferris!
```

## Development

```bash
cargo test                                  # unit + integration tests
cargo clippy --all-targets -- -D warnings   # CI fails on any warning
cargo fmt --check
pre-commit install --install-hooks --hook-type pre-commit --hook-type commit-msg
```

Commits follow [Conventional Commits](https://www.conventionalcommits.org)
(`feat:`, `fix:`, `docs:`, …) — enforced by a commit-msg hook, and the commit
type drives release versioning.

<!-- DELETE this section if you skipped release automation -->
## Releases

Every merge to `main` runs `release.yml`: git-cliff computes the next semver
from the conventional commits since the last tag (`feat` → minor, breaking →
major, else patch), pushes the `vX.Y.Z` tag, and publishes a GitHub release
with generated notes. The committed `Cargo.toml` version is never bumped by
hand — the workflow stamps the tag version in at build time.

## License

MIT OR Apache-2.0

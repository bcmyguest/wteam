# wteam

[![CI](https://github.com/bcmyguest/wteam/actions/workflows/ci.yml/badge.svg)](https://github.com/bcmyguest/wteam/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/wteam.svg)](https://crates.io/crates/wteam)

Portable worker/reviewer teams for coding agents (Claude Code, Codex, opencode, Cursor).

wteam runs planned tickets through pairs of agents. A worker implements a
ticket, a reviewer passes or fails it, and a passed ticket commits. Agents talk
through append-only mailboxes under `.wteam/`. Documentation site:
<https://bcmyguest.github.io/wteam/>.

## Install

```bash
cargo install wteam --locked
```

Or grab a static binary from the
[latest release](https://github.com/bcmyguest/wteam/releases/latest):
`x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`,
`aarch64-apple-darwin`, or `x86_64-apple-darwin`.

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

## Quickstart

```bash
wteam init                            # create .wteam/ and an effort directory
wteam pickup --from .scratch/my-effort
wteam status                          # tickets, agents, pending events
wteam ready                           # tickets whose blockers are done
wteam dispatch 01 --pair 1            # hand ticket 01 to worker.1
wteam submit 01 --summary "Done"      # worker: finished round to review
wteam verdict 01 pass --note "Nit"    # reviewer: pass with a note
```

Every command accepts `--json`. The full walkthrough is in the
[quickstart](https://bcmyguest.github.io/wteam/quickstart/).

## Commands

| Command | Purpose |
|---|---|
| `init` | Create `.wteam/` and an effort directory |
| `pickup` | Import planning output and verify it |
| `status`, `ready` | Show tickets and agents; list ready tickets |
| `dispatch`, `submit`, `verdict`, `block`, `requeue` | Move tickets through review |
| `send`, `inbox`, `wait` | Mail and event waiting |
| `graph` | Team or work graph as Mermaid (`--work`, `--team`) |
| `permissions` | Permission profile translation per CLI |
| `report`, `stop`, `restore` | Stop reports and restoring backed-up files |
| `run`, `approve`, `config`, `setup` | In progress; see the docs site |

`wteam <command> --help` lists every flag.

## team.dot

A team is a DOT graph. Override the built-in team with `.wteam/team.dot`, or
pass `--team <name|path>`:

```dot
digraph default {
  graph [max_visits=3, handoff_tokens=150000]
  orchestrator [agent="orchestrator"]
  worker       [agent="worker", count="$pairs"]
  reviewer     [agent="reviewer", count="$pairs"]
  orchestrator -> worker   [on=dispatch, kind=broadcast]
  worker -> reviewer       [on=submit, kind=pair]
  reviewer -> worker       [on=fail, kind=pair]
  reviewer -> orchestrator [on=pass, kind=fan-in]
  worker -> orchestrator   [on=blocked, kind=fan-in]
}
```

`max_visits` caps review rounds before a ticket is blocked. Attributes are in
the [reference](https://bcmyguest.github.io/wteam/reference/#team-graph-attributes).

## Agents

Each role is an agent definition, a TOML file with `name`, `cli`, and optional
`model`, `prompt`, `profile` and `extra_args`. Built-ins cover `planner`,
`orchestrator`, `worker` and `reviewer`. Override one with
`.wteam/agents/<name>.toml` or `$WTEAM_CONFIG_DIR/agents/<name>.toml`.
`cli` is `claude`, `codex`, `opencode` or `fake` (used by tests); `cursor` is in progress.

## Permissions

Profiles are Claude permission rules: `control`, `review`, `edit` and `full`.
wteam translates each one per CLI. Some translations are lossy, for example
Codex cannot deny reads of specific paths. The matrix and reasons are in the
[permissions guide](https://bcmyguest.github.io/wteam/permissions/).

## Layout

```
.wteam/.gitignore            self-ignoring
.wteam/team.dot              optional team override
.wteam/agents/, profiles/    optional agent and profile overrides
.wteam/<slug>/spec.md
.wteam/<slug>/tickets/NN-name.md
.wteam/<slug>/state.json
.wteam/<slug>/mail/<agent>.md
.wteam/<slug>/reviews/NN.md
.wteam/<slug>/queue.md
.wteam/<slug>/handoffs/, logs/, backup/, worktrees/
```

Module map and sequence diagrams: [architecture](https://bcmyguest.github.io/wteam/architecture/).

## Development

```bash
cargo test                                  # unit + integration tests
cargo clippy --all-targets -- -D warnings   # CI fails on any warning
cargo fmt --check
pre-commit install --install-hooks --hook-type pre-commit --hook-type commit-msg
```

The docs site lives in `site/` (Astro + Starlight):

```bash
cd site && pnpm install && pnpm build
```

The demo GIF (`site/public/demo.gif`) is rendered from `site/demo.tape` with
`vhs`. TODO: generate it once `vhs` is installed; until then the landing page
shows no GIF.

Commits follow [Conventional Commits](https://www.conventionalcommits.org)
(`feat:`, `fix:`, `docs:`, …), enforced by a commit-msg hook. The commit type
drives release versioning.

## Releases

Every merge to `main` runs `release.yml`: git-cliff computes the next semver
from the conventional commits since the last tag (`feat` → minor, breaking →
major, else patch), pushes the `vX.Y.Z` tag, and publishes a GitHub release
with generated notes. The committed `Cargo.toml` version is never bumped by
hand; the workflow stamps the tag version in at build time.

## License

MIT OR Apache-2.0

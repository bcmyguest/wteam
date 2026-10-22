# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with
code in this repository.

wteam: Portable worker/reviewer teams for coding agents (Claude Code, Codex, opencode, Cursor).

## Architecture

`src/lib.rs` holds the logic; `src/main.rs` only does I/O. Module map:

- `cli.rs` parses arguments; `commands/` has one file per subcommand group, each returning `Out` (text and JSON).
- `engine.rs` runs the ticket lifecycle (dispatch, submit, verdict, block). Mutate `state.json` only through `state::with_state`.
- `mailbox.rs` is the append-only mail format: a marker line, headers, a blank line, then exactly `length` bytes. Parsers slice by length and never scan bodies.
- `paths.rs` owns every path under `.wteam/`.
- `adapters/` builds per-CLI commands (`claude`, `codex`, `opencode`, `pi`, `fake`). `cursor` is not implemented yet.
- `permissions/` translates profiles per CLI. `team/` parses team DOT graphs.
- `watcher.rs` is the `wteam run` loop (handoff, budget, stop reasons). `transport/resume.rs` holds self-healing: silent-turn re-prompt, retries.
- `backup.rs` is implemented (used by `transport/resume.rs` writes, `stop` and `restore`). `worktree.rs` is a stub (`--isolation worktree`, ticket 14), as is `transport/herdr.rs`; `config` and `setup` (ticket 13) too: `bail!("not implemented…")`.

`architecture.md` at the repo root is the shared contract; the docs site renders the
module map and sequence diagrams from it (`site/src/content/docs/architecture.md`).

Tests are black-box: `tests/<topic>.rs` drives the binary through `tests/support/mod.rs`
(`Repo`, `ticket`, `agent_script`, `use_fake_agents`, `wteam()`, `wait_for`). Pure
parsers also have unit tests beside the code. Fake agents: the `fake` adapter runs
`sh $WTEAM_FAKE_DIR/<role>.sh`.

Conventions: `anyhow::Result`, no `unwrap()` outside tests, every command supports
`--json`, no new dependencies without a reason.

## Commands

```bash
cargo test                                  # full suite: unit + integration
cargo clippy --all-targets -- -D warnings   # CI fails on any warning
cargo fmt --check
cargo build --release
cd site && pnpm install && pnpm build       # docs site (Astro + Starlight)
```

wteam ticket commands (orchestrator, `src/commands/tickets.rs`):
`dispatch NN [--pair N]`, `requeue NN [--note TEXT]` (blocked tickets only: back to todo,
review rounds reset, note goes into the next dispatch brief), `approve NN` (human gate).

`tests/docs.rs` checks that every `--flag` in README.md appears in some `wteam <cmd> --help`
output (or is on its allowlist of other tools' flags). Add new README flags only after they
exist in the CLI. Flags of other tools in README (for example `claude --setting-sources`)
fail the test; describe them in prose or add them to `OTHER_TOOLS`.

Pre-commit enforces Conventional Commits (`feat:`, `fix:`, `docs:`, …) via a
commit-msg hook; install with
`pre-commit install --install-hooks --hook-type pre-commit --hook-type commit-msg`.
Commit types drive release versioning, so pick them deliberately.

## Releases

Every merge to `main` runs `.github/workflows/release.yml`: git-cliff computes
the next semver from conventional commits (`feat` → minor, breaking → major,
else patch), pushes the `vX.Y.Z` tag, builds a static binary per supported
target, publishes a GitHub release, and publishes to crates.io via Trusted
Publishing (OIDC; the crate's crates.io settings must list this repo +
`release.yml` as a trusted publisher or the publish step fails the run — the
tag and GitHub release land first regardless). Running the workflow manually
(`workflow_dispatch`) skips the release half and (re)publishes the latest
existing tag to crates.io, e.g. to backfill after a failed publish. The
committed `Cargo.toml` version is never bumped — the workflow stamps the tag
version in at build time. Don't bump the version in PRs.

The docs site deploys through `.github/workflows/pages.yml` on changes under `site/`.

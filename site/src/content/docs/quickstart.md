---
title: Quickstart
description: Initialise an effort, check its tickets and drive one ticket through review.
---

Requirements: a git repository on macOS or Linux (WSL works).

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/bcmyguest/wteam/main/install.sh | sh
```

The script installs the release binary for your platform into `~/.local/bin` after
checking its SHA-256.
From a clone: `sh install.sh`.

Or build from a checkout:

```bash
git clone https://github.com/bcmyguest/wteam
cd wteam
cargo build --release   # binary at target/release/wteam
```

## Create an effort

An effort is a directory under `.wteam/` that holds a spec, tickets, mailboxes
and state. Create `.wteam/` and an effort directory:

```bash
wteam init
```

Import the planning output (a `.scratch/<slug>` directory from `/to-tickets`)
and verify it:

```bash
wteam pickup --from .scratch/my-effort
```

Without `--from`, `pickup` verifies `.wteam/<slug>` in place.

## Inspect the work

```bash
wteam status          # tickets, agents and pending events
wteam ready           # tickets whose blockers are done
wteam graph           # team graph as Mermaid
wteam graph --work    # work graph as Mermaid
```

Every command accepts `--json` for machine-readable output.

## Choose an agent CLI

Override built-in roles with `.wteam/agents/<name>.toml` (`cli`, `model`, `profile`).
Supported headless CLIs: **Claude Code**, **Codex**, **OpenCode**, **Pi** (experimental —
use a sandbox; no profile enforcement), and **fake** (tests). **Cursor** is in progress.
Example OpenCode worker:

```toml
name = "worker"
cli = "opencode"
model = "anthropic/claude-sonnet-4-20250514"
profile = "edit"
prompt = "worker"
```

Run `wteam permissions --profile edit --cli opencode` before your first run. See
[Architecture](../architecture/#agent-adapters) and [Permissions](../permissions/).

## Move one ticket through review

The orchestrator dispatches a ready ticket to a worker pair:

```bash
wteam dispatch 01 --pair 1
```

The worker reports a finished round, or a blocker:

```bash
wteam submit 01 --summary "Added the parser and its tests"
wteam block 01 --reason "Upstream API missing"
```

The reviewer records the verdict. A `fail` needs at least one `--finding`:

```bash
wteam verdict 01 pass --note "Consider a shorter name"
wteam verdict 01 fail --finding "Parser ignores CRLF"
```

Agents read their mail with `wteam inbox` and message each other with
`wteam send <agent> "<text>"`. A blocked ticket returns to todo with `wteam requeue 01 --note "<guidance>"`.

## Run the team

To drive every ready ticket through the team until nothing is runnable:

```bash
wteam run --pairs 2
```

`run` starts the orchestrator and feeds it events until the run stops, then prints a
stop report. Use `--budget-tokens` and `--max-turns` to cap a run. Human gates pass
with `wteam approve <ticket>`. See [Concepts](../concepts/#self-healing) for how
turns recover from failures.

---
title: Cursor
description: wteam adapter for headless Cursor agent with stream-json output.
---

**CLI name:** `cursor` · **Status:** in progress

## Prerequisites

Install the [Cursor CLI](https://cursor.com/docs/cli/overview) so the `agent`
binary is on `PATH` (or set `WTEAM_CURSOR_BIN` to another name, e.g.
`cursor-agent`).

## Agent config

```toml
cli = "cursor"
model = "gpt-5.2-codex"   # optional; depends on your Cursor build
profile = "edit"
```

## What wteam runs

Each turn is roughly:

```text
agent -p --output-format stream-json --trust [--model …] [--resume <id>] --workspace <cwd> … <prompt>
```

- **Workspace:** the turn working directory (main repo or pair worktree).
- **First turn:** role text is prepended to the user message.
- **Later turns:** `--resume <session_id>`.
- **Output:** JSONL. `result` event → text and usage (`inputTokens`, cache fields).

Permissions may add `--force` (bypass) or write `.cursor/cli.json` under the
workspace depending on profile translation.

## Permissions

Cursor has no `ask` decision — profile `ask` rules become denies (≈). Write rules
scope to path globs (for example `scratch/**` for reviewers). See
[Permissions](../permissions/) and `wteam permissions --profile edit --cli cursor`.

## Environment

| Variable | Effect |
|---|---|
| `WTEAM_CURSOR_BIN` | Agent binary name (default `agent`) |

## See also

- [Architecture — adapters](../architecture/#agent-adapters)
- [Benchmark](../benchmark/) (`BENCH_CLI=cursor` default in the shared runner)

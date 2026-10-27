---
title: Claude Code
description: wteam adapter for headless claude -p with stream-json output.
---

**CLI name:** `claude` · **Status:** supported

## Prerequisites

Install [Claude Code](https://docs.anthropic.com/en/docs/claude-code) so `claude`
is on `PATH`. Sign in the way Anthropic documents for headless use.

## Agent config

```toml
cli = "claude"
model = "sonnet"   # or opus, haiku, or a full model id
profile = "edit"
```

`extra_args` are appended after adapter flags (see [Agent TOML](../reference/#agent-toml)).

## What wteam runs

Each turn is roughly:

```text
claude -p <prompt> --output-format stream-json --verbose [--model …] [--resume <id> | --append-system-prompt <role>] …
```

- **First turn:** role text from `roles/<role>.md` goes in `--append-system-prompt`.
- **Later turns:** `--resume <session_id>` from `state.json`.
- **Output:** JSONL on stdout. Session id from any event; assistant text from the
  final `result` event; context from the last top-level `assistant` usage.

Turn logs: `.wteam/<slug>/logs/<agent>-<turn>.log`.

## Permissions

Claude is the reference implementation for profiles: rules map to `--settings` JSON
and a permission mode. `dontAsk` denies anything not explicitly allowed.

Run `wteam permissions --profile edit --cli claude` before your first real run.
Matrix detail: [Permissions](../permissions/).

## Environment

| Variable | Effect |
|---|---|
| `WTEAM_CLAUDE_USER_SETTINGS=1` | Allow user hooks, skills and permission rules (default: project + local only, slash commands off) |

## See also

- [Architecture — adapters](../architecture/#agent-adapters)
- [Benchmark](../benchmark/) (Commit0 baseline uses Claude)

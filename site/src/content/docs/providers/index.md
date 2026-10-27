---
title: Providers
description: Headless CLI adapters wteam can drive — install, config, permissions and session resume per provider.
---

Each agent in `.wteam/agents/*.toml` names a `cli`. wteam builds one headless
command per turn, parses JSON(L) on stdout, and stores `session_id` and
`context_tokens` in `state.json`. Profiles are written in Claude permission
syntax; see [Permissions](../permissions/) for how each CLI maps them.

| CLI | Status | Page |
|---|---|---|
| `claude` | Supported | [Claude Code](./claude/) |
| `codex` | Supported | [Codex](./codex/) |
| `opencode` | Supported | [OpenCode](./opencode/) |
| `pi` | Experimental (sandbox only) | [Pi](./pi/) |
| `cursor` | In progress | [Cursor](./cursor/) |
| `fake` | Tests only | — runs `sh $WTEAM_FAKE_DIR/<role>.sh` |

Pick a provider page for install hints, TOML examples, resume flags, and
permission quirks. Shared adapter behaviour (logs, shims, fixtures) is in
[Architecture](../architecture/#agent-adapters).

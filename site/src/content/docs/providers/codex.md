---
title: Codex
description: wteam adapter for codex exec --json with sandbox-aware permissions.
---

**CLI name:** `codex` · **Status:** supported

## Prerequisites

Install OpenAI [Codex CLI](https://github.com/openai/codex) so `codex` is on
`PATH`. Session rollouts live under `$CODEX_HOME/sessions` (default
`~/.codex`).

## Agent config

```toml
cli = "codex"
model = "gpt-5.2-codex"   # example; use whatever your codex build accepts
profile = "edit"
```

## What wteam runs

New thread:

```text
codex exec --json [-m <model>] … <prompt-with-optional-role-prepended>
```

Resume:

```text
codex exec resume --json … <session_id> <prompt>
```

- **Role prompt:** prepended to the user message on the **first** turn only (resume turns send mail only).
- **Stdin:** empty string (codex expects a TTY-shaped invocation).
- **Output:** JSONL. `thread.started` → session id; last `agent_message` → text;
  `turn.failed` fails the turn.
- **Context tokens:** read from the newest `rollout-*-<id>.jsonl` under Codex’s
  session dir (`token_count` events), because streamed usage is cumulative.

## Permissions

Codex uses a **sandbox**, not path rules. Read vs edit profiles become
read-only vs workspace-write modes (≈ in the [permissions matrix](../permissions/)).
Any allowed edit can widen the whole workspace — planners and reviewers on Codex
are less tightly scoped than on Claude.

wteam always adds a config override so `<repo>/.wteam` is writable inside the
sandbox (agents call `wteam` during turns). Workers in git worktrees still
anchor `.wteam` on the main repo via `WTEAM_ROOT`.

## See also

- [Architecture](../architecture/)
- [Permissions](../permissions/)

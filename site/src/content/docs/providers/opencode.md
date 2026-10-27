---
title: OpenCode
description: wteam adapter for opencode run --format json.
---

**CLI name:** `opencode` · **Status:** supported

## Prerequisites

Install [OpenCode](https://opencode.ai/) so `opencode` is on `PATH`. Models use
`provider/model` syntax (for example `anthropic/claude-sonnet-4-20250514` or a
free tier model for smoke tests).

## Agent config

```toml
cli = "opencode"
model = "anthropic/claude-sonnet-4-20250514"
profile = "edit"
```

## What wteam runs

```text
opencode run --format json [-m <model>] [-s <session_id>] … <prompt>
```

- **First turn:** role text is **prepended** to the user message (OpenCode has no
  separate system slot in this adapter).
- **Later turns:** `-s <session_id>`.
- **Output:** JSONL on stdout. `sessionID`, `text` parts, `step_finish` for
  context (`tokens.input + tokens.cache.read`).
- **Truncated streams:** if final `text` events are missing, wteam recovers from
  `part_delta` (`partType: text`).
- **`error` events** fail the turn.

## Permissions

Non-`full` profiles set `OPENCODE_PERMISSION` JSON from translated rules; `full`
adds `--auto`. Path rules are enforced similarly to Claude (catch-all deny outside
allowed paths).

```bash
wteam permissions --profile edit --cli opencode
```

## Maintaining fixtures

Golden JSONL for tests lives under `tests/fixtures/adapters/opencode/`. Refresh
from a live CLI:

```bash
tests/smoke/capture-adapter-fixtures.sh
```

Optional: `WTEAM_OPENCODE_SMOKE_MODEL` (default `github-copilot/gemini-3.8-flash`).

## See also

- [Architecture — adapters](../architecture/#agent-adapters)
- [Permissions](../permissions/)

---
title: Pi
description: wteam adapter for pi --mode json (experimental).
---

**CLI name:** `pi` · **Status:** experimental — use an OS or container sandbox

## Prerequisites

Install the Pi coding agent (`pi` on `PATH`, package
[`@earendil-works/pi-coding-agent`](https://www.npmjs.com/package/@earendil-works/pi-coding-agent)
or your distro’s equivalent). Credentials and model access are your responsibility.

## Agent config

```toml
cli = "pi"
model = "claude-sonnet-4-20250514"   # passed as --model
profile = "full"   # only profile wteam can enforce on pi
```

Treat non-`full` profiles as **documentation only**: Pi has no permission API.

## What wteam runs

```text
pi --mode json [--model M] [--session ID | --append-system-prompt ROLE] … -- <prompt>
```

- **First turn:** `--append-system-prompt` with the role file.
- **Later turns:** `--session <id>`.
- **Output:** JSONL. Session header → id; last assistant `message_end` → text and
  usage. `stopReason` `error` or `aborted` fails the turn.

## Permissions

Every profile rule except `full` shows as unsupported in
`wteam permissions --cli pi`. Pi runs tools unrestricted — **run it inside a
sandbox** (container, VM, or restricted user) and assume `full` behaviour.

## Smoke tests

Pi fixtures are optional in CI:

```bash
WTEAM_PI_SMOKE=1 tests/smoke/capture-adapter-fixtures.sh
```

## See also

- [Architecture](../architecture/)
- [Permissions](../permissions/)

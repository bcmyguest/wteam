Adapter fixtures for `pi` (`@earendil-works/pi-coding-agent`).

Source: `github.com/badlogic/pi-mono`, commit `58c0b02ce552adab76014e7ed712ab7981359aa8`, read at tool version 1.4.0:

- `packages/coding-agent/docs/cli.md`: `--mode json`, `--model`, `--session <path|id>`, `--append-system-prompt`, `--`.
- `packages/coding-agent/docs/json.md`: JSONL framing, session header, `message_update`/`message_end`, `agent_settled`.
- `packages/coding-agent/docs/message-types.md`: `AssistantMessage` (`stopReason`, `errorMessage`, `usage`), `ToolCall`, `TextContent`.
- `packages/coding-agent/docs/session-format.md`: `SessionHeader` (`id` is the session id).

**Experimental:** wteam’s Pi adapter is best-effort; run Pi in a sandbox (no permission mapping). Refresh `first-turn.jsonl` with `WTEAM_PI_SMOKE=1 tests/smoke/capture-adapter-fixtures.sh` when credentials are available.

Unless captured live, fixtures are CONSTRUCTED from the documented shapes:

- `first-turn.jsonl`: CONSTRUCTED. One tool call, then a final text message. Usage numbers are invented.
- `error.jsonl`: CONSTRUCTED. Assistant message with `stopReason: "error"`.
- `malformed.jsonl`: hand-written. Second line is truncated JSON.

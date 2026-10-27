Adapter fixtures.

- `claude/first-turn.jsonl`: recorded from `claude -p "say ok" --model haiku --output-format stream-json --verbose` (2.1.307). Hook, rate-limit and skill-text events dropped, `init` trimmed, thinking signature and cwd redacted.
- `codex/usage-limit.jsonl`: recorded from `codex exec --json --skip-git-repo-check "say ok"` (0.156.1); the account had hit its usage limit, so it is the real failure shape.
- `codex/first-turn.jsonl`: CONSTRUCTED from the documented event formats (no successful live run was possible).
- `codex/home/`: constructed `$CODEX_HOME` with rollout files (`token_count` events as seen in real rollouts); the `10/20` file is older and must lose.
- `opencode/first-turn.jsonl`: CONSTRUCTED from the export format unless refreshed by `tests/smoke/capture-adapter-fixtures.sh` (OpenCode live smoke).
- `opencode/truncated-delta.jsonl`, `opencode/error-event.jsonl`: hand-written parser edge cases.
- `*/malformed.jsonl`, `claude/error.jsonl`: hand-written error cases.

# Brownfield planning contract (for planners and plan-review)

Split the task into **2–6 tickets** with a tracer that lands `dispatch --fill` first.
Name concrete `Files:` and `Verify:` lines workers can run in one shell command.

## Verify lines (use these patterns)

| Area | Suggested Verify |
|------|----------------|
| CLI / engine / fill JSON | `cargo test --test parallel fill_` |
| Watcher liveness + fill hints | `cargo test --test watcher` (whole crate, not one substring) |
| Orchestrator role | `cargo test --test roles orchestrator_runs_one_command` |
| Git lock reads | `cargo test pickup::` or tests you add in `tests/pickup.rs` |
| Docs (README / CLAUDE) | `cargo test --test docs` |
| **Full outcome** (one ticket or spec) | `sh .wteam/bench/hidden_gate.sh` |

Do **not** use a single-test substring (e.g. `cargo test one_dispatch_per_turn`) as
the only Verify on a ticket — use `cargo test --test watcher` or the hidden gate.

## Ticket graph

- **01**: tracer — `--fill` in CLI/engine; blocks others that need it.
- **02–04**: watcher, orchestrator role, worktree locks (fan out after 01 where possible).
- **05**: docs + any integration; **Verify:** should include `sh .wteam/bench/hidden_gate.sh`.

Spec **Outcome** should match `task.md` (including JSON `dispatched` / `error` fields).

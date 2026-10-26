# Brownfield task: bulk dispatch and watcher liveness

Extend the existing wteam implementation without changing its public architecture.

## Required behavior

1. `wteam dispatch` accepts either one ticket id or `--fill`, never both. `--fill`
   dispatches every currently dispatchable ready ticket to idle pairs, lowest id
   first, and prints `nothing to dispatch` when no ticket can start.
2. Text output lists every ticket that started. For `dispatch --fill --json`,
   output `{"dispatched":[{"ticket":"01","agent":"worker.1"},...]}`: the
   `dispatched` array contains every ticket started by that invocation in order,
   with its assigned agent. When no ticket can start, exit successfully, print
   `nothing to dispatch` in text mode, and output `{"dispatched":[]}` in JSON mode.
   If a later dispatch fails, earlier dispatches remain active; report them in
   text mode and exit nonzero with `{"error":"<message>","dispatched":[...]}`
   in JSON mode.
3. `wteam run` must not stop while a non-orchestrator agent is busy or a ticket is
   doing or under review. When a pair becomes free, the orchestrator gets another
   chance to fill it. Stop with a clear reason after two consecutive orchestrator
   turns leave dispatchable tickets idle.
4. The orchestrator prompt uses one `wteam dispatch --fill` command rather than a
   chain of individual dispatch commands. It runs one shell command at a time.
5. Git reads that can race with worktree rebases must disable optional locks.
6. Update README.md and CLAUDE.md for the command and operational behavior.

Preserve existing behavior and conventions. Run `cargo fmt --check` and the full
Rust test suite before finishing. Do not inspect Git history or retrieve another
implementation.

Land all changes on the **main** branch (normal commits). Use the `wteam` binary
on PATH; do not build or run wteam from this checkout with `cargo`.

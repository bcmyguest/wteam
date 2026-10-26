---
name: refine
description: >-
  Analyze a stopped wteam or brownfield run trajectory (tokens, grade gaps,
  unlanded worktrees) and apply harness-safe fixes. Use after a bench arm fails
  or `wteam run` stops early.
---

# Refine a run

Turn a failed bench or team run into concrete fixes: fewer wasted tokens, higher
pass rate on the hidden gate.

## When to use

- After `bench/brownfield/run_*.sh` or `wteam run` stops with blocked tickets or grade FAIL.
- Before re-running an expensive agent arm — refine is cheap.

## Analyze (always first)

From the **repo** that was run (not the wteam source tree unless that was the target):

```bash
export WTEAM=/path/to/wteam/target/release/wteam
wteam --root <repo> --slug bench refine \
  --logs <repo>.logs \
  --grade <repo>.logs/grade.txt \
  --run-log <repo>.logs/run.txt \
  --json
```

Read `recommendations` (priority 1 first). Common patterns:

| Signal | Meaning | Fix |
|--------|---------|-----|
| `stale-tree-wteam` / cursor adapter blocked | Agents used ba759a4 `wteam` from PATH, not release | `ensure_bench_wteam` / `PATH=.../target/release:$PATH` |
| `unlanded-worktrees` | Commits only under `.wteam/.../worktrees/pairN` | Submit/review/merge to **main** before grade |
| `fill_*` grade failures | `--fill` not on graded tree | Port `engine::fill`, CLI, `tests/parallel.rs` to main |
| High worker tokens, low progress | Exploration / rework | Tighter tickets, tracer ticket, `dispatch --fill` in orchestrator role |

## Apply safe infra fixes

```bash
wteam --root <repo> --slug bench refine ... --apply
```

Requeues tickets blocked on stale cursor errors with a retry note. Does **not** edit source.

## Brownfield loop

```bash
sh bench/brownfield/refine_loop.sh <repo-dir> <log-dir> [rounds]
```

Runs refine → re-grade → one agent pass per round until grade passes or rounds exhaust.

## Agent pass

When refine reports priority-1 **implementation** or **unlanded-worktrees**:

1. Put release wteam first on `PATH`.
2. Land tracer ticket (01) on **main** before docs/watcher tickets.
3. Match canonical API: `fill()`, `FillError`, pinned `tests/parallel.rs` — not ad-hoc `dispatch_fill.rs`.
4. Update `roles/orchestrator.md` (one command per shell, `wteam dispatch --fill`).
5. Run `sh .wteam/bench/hidden_gate.sh` when present, else `cargo fmt --check` && `cargo test`.

Do not read git history or grader commits (`audit.sh` flags leaks).

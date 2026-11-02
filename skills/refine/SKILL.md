---
name: refine
description: >-
  Change how an autonomous agent pipeline behaves based on its observed runs,
  with the smallest lever: a stopped or failing wteam run or bench arm, or any
  other harness (workflow engine, benchmark runner) whose agents loop, stall,
  skip a tool, make oversized tasks, or fail a stage. Not for crashes in the
  system the agents work on (diagnose the failure there instead).
---

# Refine a run

Turn a failed or wasteful agent run into concrete fixes: fewer wasted tokens,
higher pass rate, and a rerun that shows the effect.

## When to use

- After `bench/brownfield/run_*.sh` or `wteam run` stops with blocked tickets or grade FAIL.
- When agents in any pipeline loop, stall, skip a tool, take oversized tasks, or fail a stage.
- Before re-running an expensive agent arm — refine is cheap.

## Method (any pipeline)

1. **Read the run's events, not the agents' summaries.** Confirm whether the agent
   called the tool at all, and whether the call succeeded.
2. **Classify the cause:** prompt, tool availability, limit (attempts, timeout,
   budget), graph order, task size, or environment (stale binary, missing service).
3. **Pull the smallest lever first:** role prompt or config, then graph, then
   engine code. New behaviour should not need new engine code.
4. **Bound the loop:** cap attempts and timeouts; forbid steps that cost time
   without adding signal at that stage (for example, full test runs inside a
   planning node).
5. **Gate completion on verification:** a stage that can finish before its check
   runs is a graph-order bug.
6. **Steer task size:** a tracer-bullet first task, then small tasks.
7. **Remove tools that cause harm or cost** at that stage (web access, paid APIs).
8. **Rerun the failing case** and compare events and tokens before and after.
   Report what changed; a fix without a rerun is a proposal.

Signals for pipelines other than wteam: [references/other-pipelines.md](references/other-pipelines.md).

## wteam runs: analyze first

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

## wteam runs: apply safe infra fixes

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

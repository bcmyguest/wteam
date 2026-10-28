---
title: Benchmark
description: wteam against single-agent baselines on Aider's polyglot benchmark and larger multi-ticket tasks — what multi-agent coordination buys, and what it does not.
---

One run per configuration on 2026-10-22 and 2026-10-27, so treat differences of one or two exercises as noise.
The harness is in [`bench/polyglot/`](https://github.com/bcmyguest/wteam/tree/main/bench/polyglot); it uses the shared [`bench/runner/`](https://github.com/bcmyguest/wteam/tree/main/bench/runner) so you can set **`BENCH_CLI`** (`cursor`, `claude`, `codex`, …) and rerun with your models.

wteam optimizes **controlled parallel delivery and reviewable ownership**, not minimum token cost.
These suites mostly measure the latter, so a single coherent session often wins on pass rate and dollars.
The numbers below still show where wteam pays for itself: wall time under independent work, accuracy when reviewers outrank workers, and structure when every arm already passes the tests.

Fresh-context Goldfish phases (`wteam phase plan-review` / `wteam phase validate`, configurable personas) follow Google’s [Elephant-Goldfish](https://github.com/google/elephant-goldfish-model) pattern: isolate critics from Elephant chat. The orchestrator owns validation loops; the CLI only stamps state. Orthogonal to parallel workers on polyglot.

## Setup

- **Tasks:** the 34 Python exercises of [Aider's polyglot benchmark](https://github.com/Aider-AI/polyglot-benchmark),
  the hardest Exercism problems. Each is a stub plus an `INSTRUCTIONS.md`.
- **Grading:** `grade.sh` runs the **pristine** unit tests against each solution, so an agent that edits tests gains nothing.
  The reference solutions score 34/34; the untouched stubs score 0/34.
- **Two modes:**
  - **visible**: the unit tests sit next to the stub, and wteam tickets carry them as `Verify:`.
  - **hidden**: the tests are removed before the agents start (Aider's protocol), so agents work from `INSTRUCTIONS.md` alone and must write their own checks.
- **Baselines:** one headless session solves all 34 (`run_single.sh` / `run_polyglot_continuous.sh`). Default **`BENCH_CLI=cursor`** today; published tables used Claude Haiku/Sonnet.
- **wteam:** one ticket per exercise, 4 worker/reviewer pairs (`run_wteam.sh`). Set `BENCH_REVIEW=on` for a real reviewer on the same CLI.
- **Matrix:** `sh bench/polyglot/run_matrix.sh <checkout> <dir> 4` runs continuous, fresh-per-ticket, and team with the same CLI config.
- **Cost:** Claude runs report `total_cost_usd`; Cursor stream-json logs feed `metrics.py` (`total_tokens`). Subscription pricing differs; ratios still compare arms fairly.

## Results

### Hidden tests (the informative mode)

| Configuration | Passed | Wall time | Cost |
|---|---|---|---|
| Single session, Sonnet | 27/34 | 214 s | $0.79 |
| Single session, Haiku | 20/34 | 400 s | $0.28 |
| wteam, Haiku workers + Haiku reviewers, model orchestrator | 20/34 | 428 s | $3.09 |
| wteam, Haiku workers + Haiku reviewers, auto dispatch | 20/34 | 334 s | $1.92 |
| wteam, Haiku workers + **Sonnet reviewers**, auto dispatch | **28/34** | 351 s | $12.23 |

On 2026-10-27, both Codex arms used `gpt-6-sol` with hidden tests and all 34 exercises. The team used four pairs, auto dispatch, and an autopass reviewer; these runs did not report cost. The single session passed 24/34 in 383 s, while wteam passed 26/34 in 613 s. The two-exercise gap comes from one run per arm and does not establish a reliable pass-rate difference. [Full results and failing exercises](https://github.com/bcmyguest/wteam/blob/main/bench/polyglot/results/hidden-codex-2026-10-27.md).

### Visible tests

Visible mode saturates: single Sonnet and single Haiku both reach 34/34 (236 s / $1.10 and 440 s / $0.47).
Hidden tests are the comparison that still discriminates.

## What it says

### Measured benefits

- **Parallel pairs cut wall time.** With four pairs, auto-dispatch wteam finished the matching hidden Haiku comparison in 334 s versus 400 s for one Haiku session, at the same 20/34 score.
- **A stronger reviewer lifts cheap workers.** Haiku workers with Sonnet reviewers scored 28/34 — one above a single Sonnet session and eight above Haiku alone. Sonnet review accounted for $10.96 of the $12.23. Built-in defaults follow this: worker `sonnet`, reviewer `opus`. Same-tier review is optional to configure, not the recommended shape.
- **Ticket-scoped work preferred on structure.** On larger greenfield tasks where every arm already passed the same hidden tests, Opus judged module pairs blind (both orders) and preferred wteam’s code in 27 of 30 comparisons, then 22 of 30 on a repeat run: smaller named helpers, one place per rule, located errors, type hints, docstrings. The edge tracked one focused worker per module; Sonnet reviewers did not add to it in the run that isolates them.

### Why multi-agent does not dominate pass rate here

The literature that says “more agents help” usually measures a different mechanism than wteam’s ticket pipeline.

1. **Sampling-and-voting ≠ worker/reviewer.**
   [*More Agents Is All You Need*](https://arxiv.org/abs/2402.05120) scales by independent solutions and a vote.
   wteam produces one implementation per ticket, then reviews it — governance, not answer diversity.
   That paper also finds that agent debate can harm code coherence when agents remix each other’s drafts.

2. **Precise greenfield specs favour one memory stream.**
   Polyglot exercises and the larger rebuilds hand every rule up front.
   A single session keeps the full spec, discoveries, and edits together.
   wteam partitions that into tickets, briefs, summaries, and diffs; same-tier reviewers see the ticket and diff, not the worker’s full trail, and under hidden tests share the worker’s missing grader signal.
   Haiku reviewing Haiku stayed at 20/34.

3. **Coordination has a tax, especially on sequential or tool-heavy work.**
   [Google Research](https://research.google/blog/towards-a-science-of-scaling-agent-systems-when-and-why-agent-systems-work/)
   ([paper](https://arxiv.org/abs/2512.08296)) finds multi-agent gains on parallelizable tasks and 39–70% *degradation* on sequential ones, plus a rising coordination cost as tool use grows.
   [Anthropic’s multi-agent research system](https://www.anthropic.com/research/multi-agent-research-system)
   reports large gains on breadth-first research, while noting coding offers less true parallelism and multi-agent runs consume far more tokens than ordinary chats.
   Polyglot’s 34 tiny tickets maximize per-ticket overhead (brief, verify, review, commit, merge) relative to useful work.

### Cost of orchestration

Model-driven dispatch spent $0.79–$0.87 per run on turns that only ran `wteam dispatch --fill`.
Auto dispatch spent $0.00–$0.05 for the same fill behavior — the auto rows above are the fair wteam comparison.

## Larger multi-ticket tasks

Polyglot exercises are small and independent, so on 2026-10-22 we also ran tasks with dependent tickets.
One run per arm; “wteam” is an Opus planner, Haiku workers and Sonnet reviewers in 4 pairs.
Every score was checked for memorized upstream code and for reads outside the run’s own repo.
Full tables and method: [`bench/results/2026-10-22.md`](https://github.com/bcmyguest/wteam/blob/main/bench/results/2026-10-22.md).

| Task | Single Haiku | wteam |
|---|---|---|
| bitstring rebuild ([Commit0](https://github.com/commit-0), 834 hidden tests) | 681, $3.55, 27 min | 684, $9.64, 51 min |
| incidentkit v1 (own 20 KB spec, 523 hidden tests) | 523, $0.16, 6 min | 523, $3.00, 11 min |
| incidentkit v2 (own 83 KB spec, five interacting subsystems, 2,399 hidden tests) | 2399, $0.77, 11 min | 2399, $7.47, 16 min |

On greenfield work from a precise written spec, a single Haiku session matched wteam’s pass counts at 2–10× lower cost.
Planning and parallel pairs did not raise the score when the agent already held every rule.
Vague goals, changes to existing code, and debugging — where a single session tends to drift — are not covered by these tables.
Cursor + Composer runs (2026-10-26): [`bench/results/2026-10-26.md`](https://github.com/bcmyguest/wteam/blob/main/bench/results/2026-10-26.md) — brownfield **single FAIL / team PASS**, Commit0 `tinydb` three-way matrix, Terminal-Bench oracle smoke.
A brownfield harness lives in [`bench/brownfield/`](https://github.com/bcmyguest/wteam/tree/main/bench/brownfield).

## Configurable runner (Commit0-style)

[`bench/commit0/`](https://github.com/bcmyguest/wteam/tree/main/bench/commit0) remains the published **Claude Code** harness (fixed CLI and scripts).
[`bench/runner/`](https://github.com/bcmyguest/wteam/tree/main/bench/runner) is **CLI-agnostic**: the same hidden-pytest tasks and three **context** arms on one agent configuration.

| Mode | Context | What it measures |
|---|---|---|
| **continuous** | One session, full goal | Single long chat (optional saved `PLAN=` in the same session) |
| **fresh** | New session per ticket | Same model/cli, no resume between tickets |
| **team** | wteam pairs | Planner → pickup → `wteam run` (same layout as commit0 wteam) |

Entry points:

```bash
sh bench/runner/run.sh continuous <lib> <dir> [model]
sh bench/runner/run.sh fresh       <lib> <dir> [model]
sh bench/runner/run.sh team        <lib> <dir> <pairs> [model]
# Three models (planner / worker / reviewer), like commit0:
sh bench/runner/run_team.sh <lib> <dir> <planner> <worker> <reviewer> <pairs>
BENCH_LIBS=tinydb sh bench/runner/run_matrix.sh /tmp/matrix-three-way
```

| Env | Meaning |
|---|---|
| `BENCH_CLI` | wteam adapter name: `cursor` (default), `claude`, `codex`, `opencode`, `pi`, … |
| `BENCH_MODEL` | Default model for all roles |
| `BENCH_PLANNER_MODEL` / `BENCH_WORKER_MODEL` / `BENCH_REVIEWER_MODEL` | Team overrides |
| `BENCH_REVIEW` | `autopass` (fake reviewer) or `on` / `real` (same CLI as workers) |
| `BENCH_PAIRS` | Team pair count (default 4) |
| `WTEAM_BENCH_ISOLATION` | Team isolation (runner default `worktree`; brownfield often `none`) |
| `PLAN=<dir>` | Saved `spec.md` + tickets — continuous loads all in one prompt; fresh one session per ticket |
| `CODEGRAPH`, `SHAPE=coupled`, `OVERLAP`, `RUN_TIMEOUT` | Same as commit0 wteam |

Metrics land in `<dir>.logs/metrics.txt` (`bench_mode`, `bench_cli`, grade `RESULT`, tokens or `cost_usd`).
Full detail: [`bench/runner/README.md`](https://github.com/bcmyguest/wteam/blob/main/bench/runner/README.md).

[`bench/agent/`](https://github.com/bcmyguest/wteam/tree/main/bench/agent) and thin wrappers under
[`bench/commit0/`](https://github.com/bcmyguest/wteam/tree/main/bench/commit0) and
[`bench/brownfield/`](https://github.com/bcmyguest/wteam/tree/main/bench/brownfield) **delegate** to
`bench/runner/` (`run_single.sh` → continuous, `run_wteam.sh` → team). Brownfield adds
`sh bench/brownfield/run.sh <continuous|fresh|team> …` on the internal task.

[`bench/BENCH_OPTIONS.md`](https://github.com/bcmyguest/wteam/blob/main/bench/BENCH_OPTIONS.md) notes
which external suites are saturated vs still useful; [`bench/terminal-bench/`](https://github.com/bcmyguest/wteam/tree/main/bench/terminal-bench) targets Harbor Terminal-Bench 4.0.

### Terminal-Bench 4.0 status

A [ten-task Codex Luna team attempt](https://github.com/bcmyguest/wteam/blob/main/bench/results/2026-10-28-terminal-bench.md) returned a raw Harbor reward of 0/10. Codex usage limits and harness failures interrupted most trials; the matching single-agent arm has not run. We do not report a single-versus-team score from this attempt.

## Architectural benefits (not scored by these suites)

These are product properties the benchmarks do not price:

- Explicit ticket ownership and bounded change scopes
- Independent verification and configurable review chains
- Worktree isolation and controlled landing
- Persistent tickets, reviews, logs, handoffs, and resumable sessions
- Fault containment, retry/replan paths, and human approval gates
- Different models, CLIs, prompts, and permissions per role

## Caveats

- One run each. Haiku’s run-to-run variance on this suite is unmeasured.
- Runs shared one account’s rate limits and sometimes ran concurrently.
- Blind code-quality judgments used Opus on same-test greenfield modules; a brownfield replication with held-out tests is still open.

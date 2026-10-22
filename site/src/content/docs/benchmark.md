---
title: Benchmark
description: wteam against single-agent baselines on Aider's polyglot benchmark (Python), with every result, including the unflattering ones.
---

One run per configuration on 2026-10-22, so treat differences of one or two exercises as noise.
The harness is in [`bench/polyglot/`](https://github.com/bcmyguest/wteam/tree/main/bench/polyglot); rerun it with your own models.

## Setup

- **Tasks:** the 34 Python exercises of [Aider's polyglot benchmark](https://github.com/Aider-AI/polyglot-benchmark),
  the hardest Exercism problems. Each is a stub plus an `INSTRUCTIONS.md`.
- **Grading:** `grade.sh` runs the **pristine** unit tests against each solution, so an agent that edits tests gains nothing.
  The reference solutions score 34/34; the untouched stubs score 0/34.
- **Two modes:**
  - **visible**: the unit tests sit next to the stub, and wteam tickets carry them as `Verify:`.
  - **hidden**: the tests are removed before the agents start (Aider's protocol), so agents work from `INSTRUCTIONS.md` alone and must write their own checks.
- **Baselines:** one headless `claude -p` session solves all 34, with the same isolation and permission shape as wteam workers.
- **wteam:** one ticket per exercise, 4 worker/reviewer pairs, worktree isolation, built-in profiles.
- **Cost:** the `total_cost_usd` Claude Code reports (API-equivalent). Subscription users pay differently, but the ratios hold.

## Results

### Hidden tests (the informative mode)

| Configuration | Passed | Wall time | Cost |
|---|---|---|---|
| Single session, Sonnet | 27/34 | 214 s | $0.79 |
| Single session, Haiku | 20/34 | 400 s | $0.28 |
| wteam, Haiku workers + Haiku reviewers, model orchestrator | 20/34 | 428 s | $3.09 |
| wteam, Haiku workers + Haiku reviewers, `--dispatch auto` | 20/34 | 334 s | $1.92 |
| wteam, Haiku workers + **Sonnet reviewers**, `--dispatch auto` | **28/34** | 351 s | $12.23 |

### Visible tests (saturated: every configuration can reach 34/34)

| Configuration | Passed | Wall time | Cost |
|---|---|---|---|
| Single session, Sonnet | 34/34 | 236 s | $1.10 |
| Single session, Haiku | 34/34 | 440 s | $0.47 |
| wteam, Haiku, model orchestrator | 31/34 | 231 s | $2.33 |

## What it says

- **For small independent tasks, one session is the better deal.** Polyglot is close to wteam's worst case: 34 tiny tasks,
  nothing to plan, no dependencies, and a fixed per-ticket cost for briefs, reviews and commits.
- **A same-tier reviewer adds cost, not quality.** Haiku reviewing Haiku failed almost the same exercises as Haiku alone.
  The reviewer shares the worker's blind spots.
- **A stronger reviewer lifts cheap workers.** Haiku workers with Sonnet reviewers scored 28/34, one above a single Sonnet
  session and eight above Haiku alone. In this setup the Sonnet reviewers cost $10.96 of the $12.23, so reviewer token
  use is the next thing to cut.
- **Parallel pairs are faster.** With four pairs, wteam beat the matching single Haiku session on wall time in both modes.
- **`--dispatch auto` (now the default) removes orchestrator cost.** Model-driven dispatch spent $0.79–$0.87 per run on
  turns that only ran `wteam dispatch --fill`; auto dispatch spent $0.00–$0.05.

## Defects the benchmark found

Every early wteam failure was a wteam bug, not a model limit. Each became a ticket:

- **Orchestrator chained commands, so all were denied:** 4/34 → `dispatch --fill`, and the one-command rule for the orchestrator (ticket 27).
- **Watcher stopped while pairs were still busy** (ticket 27).
- **`git status` raced a rebase on `index.lock`:** fixed with `--no-optional-locks` (ticket 27).
- **Workers couldn't run `python3` for self-checks, and blocked tickets wedged their pairs:** 1/34 → ticket 28.
- **Shell-built scratch files and multi-line commands were denied:** 13/34 → ticket 30.

## Caveats

- One run each. Haiku's run-to-run variance on this suite is unmeasured.
- Runs shared one account's rate limits and sometimes ran concurrently.
- A benchmark of one large effort with dependent tickets, where wteam's planning, handoff and parallelism should matter,
  is still to come.

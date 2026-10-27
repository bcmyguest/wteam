---
name: wteam
description: Run a team of worker/reviewer pairs through a multi-ticket effort with wteam. Use when the user asks for wteam, a team, worker-reviewer pairs, parallel agents on tickets, or a multi-ticket build.
---

Drive `wteam` through these steps. Finish each step's completion criterion before starting the next.

## 1. Intake

Send one plain-text message asking three things: the goal, the complexity (S, M or L), and the pair count. Wait for the reply; type the questions into the message, never into an interactive question tool.

Derive `<slug>` from the goal: kebab-case, at most 4 words. Pass `--slug <slug>` to every `wteam` command that follows.

Done when the user has answered all three and you hold the slug.

## 2. Plan

Planning already happened (tickets in `.scratch/<slug>`): run `wteam pickup --from .scratch/<slug> --slug <slug>`.

Otherwise spawn the planner with the prompt from `wteam role planner` (stdout; `--json` returns it as `text`), plus the goal and the slug. Model: sonnet (or the harness's medium model) for S and M, opus (the big model) for L.

- Claude Code: spawn a subagent whose prompt is the role prompt.
- Codex: `codex exec -m <model> "<role prompt>"`.
- opencode: `opencode run --format json -m <provider/model> "<role prompt>"` (wteam prepends the role on the first turn instead). Profiles map to `OPENCODE_PERMISSION`; `full` adds `--auto`.
- pi: **experimental** — run in a sandbox; wteam cannot enforce profiles. `pi --mode json --model <model> -- "<role prompt>"`. The `--` keeps a prompt that starts with `-` from parsing as a flag (see `src/adapters/pi.rs`).
- Any other harness: follow the role prompt in-session.

Done when `wteam pickup --slug <slug>` exits 0 with no problems listed.

## 3. Plan review (Goldfish)

Before Confirm, stress-test the plan with **fresh-context** reviewers (same idea as Google’s [Elephant-Goldfish Model](https://github.com/google/elephant-goldfish-model): deep planner context, then isolated critics on the artifact).

Default chain (override agent TOMLs under `.wteam/agents/` like any role):

1. `spec-comprehension` — can a stranger reconstruct the plan from `spec.md` + tickets alone?
2. `spec-critic` — edges, Verify/Files, needless serialisation, weeds
3. `spec-readiness` — go/no-go for workers

Run `wteam phase plan-review --slug <slug>` (alias: `wteam plan-review`). On FAIL: show findings, spawn the planner to rewrite with **minimum** edits (only what findings name; no scope expansion), `wteam pickup` again, re-run (cap two rewrite loops). On PASS, a stamp lands at `.wteam/<slug>/plan-reviews/PASSED.md`.

Done when the phase exits 0.

## 4. Confirm

Show the `wteam pickup --slug <slug>` output and `wteam permissions --slug <slug>` for the team, then wait for the user.

Done when the user says go.

## 5. Run

Run `wteam run --pairs N --slug <slug>`. It runs long: use a background task where the harness offers one, else the foreground. The watcher in `wteam run` waits and feeds the orchestrator; never call `wteam wait` yourself. Add `--budget-tokens <n>` when the user sets a token cap. `--dispatch auto` (the default) has the watcher dispatch ready tickets itself and wakes the orchestrator only for blocked tickets and human gates. `--dispatch model` has the orchestrator dispatch every ticket, as before.

The orchestrator (Elephant) may call `wteam phase validate` while the run is live. That is judgment, not a watcher hook.

Done when `wteam run` exits and prints the stop report.

## 6. Validate (backstop)

If `.wteam/<slug>/phases/validate/PASSED.md` is missing after Run, run `wteam phase validate --slug <slug>` once (local → mid → out Goldfish). On FAIL: `wteam requeue NN --note "<finding>"` for named tickets, short `wteam run`, re-validate (cap two). The orchestrator should have done this already; the skill only backs it.

Done when validate PASSED exists or the user accepts remaining findings.

## 7. Relay

Relay the stop report (`wteam report --slug <slug>` reprints it) with its stop reason and recommended actions. Ask how to proceed.

Blocked tickets go back to todo only with the user's agreement. Then run `wteam requeue NN --slug <slug> --note "<guidance>"`. The note goes into the next dispatch brief. `requeue` accepts only blocked tickets and resets the review round count. Then run `wteam run` again.

Done when the user has chosen the next move.

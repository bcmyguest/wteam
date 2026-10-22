---
name: wteam
description: Run a team of worker/reviewer pairs through a multi-ticket effort with wteam. Use when the user asks for wteam, a team, worker-reviewer pairs, parallel agents on tickets, or a multi-ticket build.
---

Drive `wteam` through five steps. Finish each step's completion criterion before starting the next.

## 1. Intake

Send one plain-text message asking three things: the goal, the complexity (S, M or L), and the pair count. Wait for the reply; type the questions into the message, never into an interactive question tool.

Derive `<slug>` from the goal: kebab-case, at most 4 words. Pass `--slug <slug>` to every `wteam` command that follows.

Done when the user has answered all three and you hold the slug.

## 2. Plan

Planning already happened (tickets in `.scratch/<slug>`): run `wteam pickup --from .scratch/<slug> --slug <slug>`.

Otherwise spawn the planner with the prompt from `wteam role planner` (stdout; `--json` returns it as `text`), plus the goal and the slug. Model: sonnet (or the harness's medium model) for S and M, opus (the big model) for L.

- Claude Code: spawn a subagent whose prompt is the role prompt.
- Codex: `codex exec -m <model> "<role prompt>"`.
- opencode: `opencode run -m <provider/model> "<role prompt>"`.
- pi: `pi --mode json --model <model> -- "<role prompt>"`. The `--` keeps a prompt that starts with `-` from parsing as a flag (see `src/adapters/pi.rs`).
- Any other harness: follow the role prompt in-session.

Done when `wteam pickup --slug <slug>` exits 0 with no problems listed.

## 3. Confirm

Show the `wteam pickup --slug <slug>` output and `wteam permissions --slug <slug>` for the team, then wait for the user.

Done when the user says go.

## 4. Run

Run `wteam run --pairs N --slug <slug>`. It runs long: use a background task where the harness offers one, else the foreground. The watcher in `wteam run` waits and feeds the orchestrator; never call `wteam wait` yourself. Add `--budget-tokens <n>` when the user sets a token cap. `--dispatch auto` (the default) has the watcher dispatch ready tickets itself and wakes the orchestrator only for blocked tickets and human gates. `--dispatch model` has the orchestrator dispatch every ticket, as before.

Done when `wteam run` exits and prints the stop report.

## 5. Relay

Relay the stop report (`wteam report --slug <slug>` reprints it) with its stop reason and recommended actions. Ask how to proceed.

Blocked tickets go back to todo only with the user's agreement. Then run `wteam requeue NN --slug <slug> --note "<guidance>"`. The note goes into the next dispatch brief. `requeue` accepts only blocked tickets and resets the review round count. Then run `wteam run` again.

Done when the user has chosen the next move.

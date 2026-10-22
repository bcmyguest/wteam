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

Otherwise spawn the planner with the brief below plus the goal and the slug. Model: sonnet (or the harness's medium model) for S and M, opus (the big model) for L.

- Claude Code: spawn a subagent whose prompt is the brief.
- Codex: `codex exec -m <model> "<brief>"`.
- opencode: `opencode run -m <provider/model> "<brief>"`.
- Any other harness: follow the brief in-session.

Planner brief:

> Write `spec.md` (outcome, constraints, non-goals) and `tickets/NN-<name>.md` into `.wteam/<slug>/`. Each ticket, in the `/to-tickets` format, carries `What to build`, `Blocked by`, `Status: ready-for-agent`, `Files` (the only paths its worker may edit), `Verify` (the proving command) and checkbox acceptance criteria that a test observes. Ticket 01 is a tracer bullet: the thinnest path through every layer. Tickets with overlapping `Files` block each other. On a re-plan run, read `.wteam/<slug>/queue.md` first; turn a follow-up key into a ticket on its third sighting. Finish when `wteam pickup --slug <slug>` lists no problems.

Done when `wteam pickup --slug <slug>` exits 0 with no problems listed.

## 3. Confirm

Show the `wteam pickup --slug <slug>` output and `wteam permissions --slug <slug>` for the team, then wait for the user.

Done when the user says go.

## 4. Run

Run `wteam run --pairs N --slug <slug>`. It runs long: use a background task where the harness offers one, else the foreground. The watcher in `wteam run` waits and feeds the orchestrator; never call `wteam wait` yourself.

Done when `wteam run` exits and prints the stop report.

## 5. Relay

Relay the stop report (`wteam report --slug <slug>` reprints it) with its recommended actions. Ask how to proceed.

Done when the user has chosen the next move.

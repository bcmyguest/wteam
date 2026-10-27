---
title: Concepts
description: Roles, the team and work graphs, mailboxes and handoff.
---

## Roles

| Role | Job |
|---|---|
| `planner` | Turns the spec into tickets and answers follow-ups from the queue. |
| `orchestrator` | Dispatches ready tickets to workers and reacts to events. |
| `worker` | Implements one ticket in a pair and submits the round for review. |
| `reviewer` | Gives a `pass` or `fail` verdict on a submitted round. |

Instances are numbered: `worker.2` is the second worker. `orchestrator` and
`planner` are always `.1`. A pair is one worker and one reviewer with the same index.

Each role resolves to an **agent definition** (TOML): `cli`, optional `model`,
`profile`, and `prompt`. Built-in CLIs are Claude Code, Codex, OpenCode, Pi
(experimental — sandbox only), and `fake` for tests; Cursor is in progress.
wteam runs one headless adapter invocation per turn, resumes the stored session id
when the CLI supports it, and parses assistant text plus token usage from stdout.
See [Architecture](../architecture/#agent-adapters).

## Team graph

The team graph says who talks to whom and on which event. It is a DOT file:

```text
digraph default {
  graph [max_visits=3, handoff_tokens=150000]
  orchestrator [agent="orchestrator"]
  worker       [agent="worker", count="$pairs"]
  reviewer     [agent="reviewer", count="$pairs"]
  orchestrator -> worker   [on=dispatch, kind=broadcast]
  worker -> reviewer       [on=submit, kind=pair]
  reviewer -> worker       [on=fail, kind=pair]
  reviewer -> orchestrator [on=pass, kind=fan-in]
  worker -> orchestrator   [on=blocked, kind=fan-in]
}
```

Node shapes: `box` is an agent role, `parallelogram` is a command (`script=`),
and `hexagon` is a human gate. Edge `kind` defaults to `pair`. `count` defaults to 1.
`max_visits` caps review rounds: after that many failures, the ticket becomes `Blocked`.

### Gates

A ticket's `Verify:` commands run first at `submit`, then the command nodes on the
worker's path. A failing command (non-zero exit or timeout) returns the ticket to
the worker with the output tail as the finding. A human gate holds a passed ticket
until `wteam approve <ticket>`. A human gate cannot directly follow `submit`. A pass
path that reaches a reviewer who already passed the round blocks the ticket.

## Work graph

The work graph holds tickets and their `Blocked by` links. `wteam ready` lists
the tickets with every blocker `Done`. `wteam graph --work` draws it.

## Ticket lifecycle

1. `dispatch` sets a ready ticket to `Doing` and mails its brief to a worker.
2. `submit` moves it to `Review` and mails the reviewer the ticket, the worker's
   summary and the diff of the ticket's files.
3. `verdict pass` commits the ticket's files as `feat: <title> (wteam NN)` and
   sets it to `Done`. `verdict fail` returns it to `Doing` with the findings.
4. `block` marks the ticket `Blocked` and tells the orchestrator. `requeue` returns a
   blocked ticket to `Todo` with a note for the next dispatch brief; the review round
   count resets. Only blocked tickets can be requeued.

Full sequence: [Architecture](../architecture/#ticket-lifecycle).

## Mailboxes

Each agent has an append-only mailbox at `.wteam/<slug>/mail/<agent>.md`. A
message is a marker line, headers (`from`, `at`, optional `ticket`, `length`),
a blank line and exactly `length` bytes of body. Parsers read by length, so a
body can hold a diff or a whole ticket. `wteam inbox` shows unread messages
and `--all` shows the whole mailbox.

## Handoff

When the orchestrator's context passes `min(handoff_tokens, 0.75 × context_window)`,
`wteam run` asks it for a baton. The baton is written to `handoffs/orchestrator-<gen>.md`,
and the next generation starts with that file as its brief.

## Dispatch modes

`wteam run --dispatch auto` (the default) fills idle pairs with the watcher itself, so
dispatch costs no model turn. The orchestrator wakes only when a ticket blocks or a human
gate needs approval. `--dispatch model` keeps the orchestrator dispatching every ticket.

`--replan` (off by default) gives each blocked ticket one planner turn under `auto`. The planner
runs in the repo root with the `plan` profile, rewrites the ticket, and requeues it. A ticket
blocked again after its re-plan stays blocked, and the stop report says it was re-planned once.
The stop report and `--json` record the mode.

## Self-healing

A turn that ends while its agent still holds a ticket, with no `submit` or `block`,
gets one re-prompt. A second silent turn blocks the ticket and tells the orchestrator.
Transient adapter failures retry three times with backoff. A busy agent whose turn
process has died is reset on the next command.

## Live results

One small smoke test, not a benchmark. The work is a small Python project. Workers and
reviewers run Claude Haiku. Costs are Claude-reported.

| Run | Work | Wall time | Result | Cost |
|---|---|---|---|---|
| 2 | Tickets 01 (subtract) and 02 (clamp_diff), one pair | 16 s and 13 s | Both passed on round 1 | about $0.025 |
| 3 | Tickets 03 (multiply) and 04 (safe_divide), `wteam run --pairs 1` | 34 s | Stop report: 4 done, 0 blocked; orchestrator used 3 turns | about $0.056 |

Run 1 stalled before these numbers: agents loaded user settings and Bash denials
left the worker unable to submit. Agent isolation and the one-command rule fixed it.

## Reviews and the queue

Every verdict appends to `reviews/NN.md`, newest last. Non-blocking follow-ups
(`--follow-up key:text`) append to `queue.md` for the planner.

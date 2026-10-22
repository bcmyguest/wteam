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

:::caution[Status: in progress]
`submit` rejects a round whose next hop is a command or human gate. The parser accepts those nodes.
:::

## Work graph

The work graph holds tickets and their `Blocked by` links. `wteam ready` lists
the tickets with every blocker `Done`. `wteam graph --work` draws it.

## Ticket lifecycle

1. `dispatch` sets a ready ticket to `Doing` and mails its brief to a worker.
2. `submit` moves it to `Review` and mails the reviewer the ticket, the worker's
   summary and the diff of the ticket's files.
3. `verdict pass` commits the ticket's files as `feat: <title> (wteam NN)` and
   sets it to `Done`. `verdict fail` returns it to `Doing` with the findings.
4. `block` marks the ticket `Blocked` and tells the orchestrator.

Full sequence: [Architecture](../architecture/#ticket-lifecycle).

## Mailboxes

Each agent has an append-only mailbox at `.wteam/<slug>/mail/<agent>.md`. A
message is a marker line, headers (`from`, `at`, optional `ticket`, `length`),
a blank line and exactly `length` bytes of body. Parsers read by length, so a
body can hold a diff or a whole ticket. `wteam inbox` shows unread messages
and `--all` shows the whole mailbox.

## Handoff

When an agent's context grows past `handoff_tokens`, the orchestrator writes
`handoffs/orchestrator-<gen>.md` and starts a new generation with that file as its brief.

:::caution[Status: in progress]
Handoff generation is part of the orchestrator loop that `wteam run` drives. Not yet wired.
:::

## Reviews and the queue

Every verdict appends to `reviews/NN.md`, newest last. Non-blocking follow-ups
(`--follow-up key:text`) append to `queue.md` for the planner.

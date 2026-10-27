---
title: broadcast
description: Send the same mail to every worker (or every instance of a role).
---

In `team.dot`:

```text
orchestrator -> worker [on=dispatch, kind=broadcast]
```

## What you get

One event reaches **all** workers at once — `worker.1`, `worker.2`, … up to your
`--pairs` count (or the role’s `count=` in the graph).

The default topology uses broadcast on **dispatch** so every worker is notified
when the orchestrator (or `wteam run --dispatch auto`) assigns work. Only the
pair that actually holds the ticket should act; wteam still tracks assignee in
`state.json`.

## When to choose broadcast

- One coordinator needs to reach **every** worker with the same brief or announcement.
- You want parallel pairs woken together when tickets become ready.

Use [pair](./pair/) for submit/fail so only the right reviewer or worker is
involved. Use [fan-in](./fan-in/) when many agents should report to **one**
orchestrator or reviewer instead of copying mail to everyone.

[Back to mail routing overview](./)

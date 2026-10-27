---
title: pair
description: Route mail to the worker or reviewer with the same pair number.
---

In `team.dot`:

```text
worker -> reviewer [on=submit, kind=pair]
```

(`kind=pair` is optional — it is the default.)

## What you get

Pair routing keeps one worker and one reviewer on the same ticket round. With
`wteam run --pairs 2`:

- When **worker.1** submits, **reviewer.1** gets the review mail.
- When **worker.2** submits, **reviewer.2** gets it.
- When **reviewer.1** fails the round, mail goes back to **worker.1** (not worker.2).

That is the usual worker ↔ reviewer loop.

## When to choose pair

- Standard multi-pair runs: each pair owns its tickets independently.
- A second reviewer on the **same** pair (e.g. a security pass after the first
  reviewer) — still `kind=pair` so pair 1’s security reviewer only sees pair 1’s work.

Use [broadcast](./broadcast/) when everyone on a role must see the same message.
Use [fan-in](./fan-in/) when many senders should notify a single coordinator.

[Back to mail routing overview](./)

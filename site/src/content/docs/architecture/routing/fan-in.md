---
title: fan-in
description: Route mail from any pair to a single coordinator (usually .1).
---

In `team.dot` the spelling is **`fan-in`** (with a hyphen):

```text
reviewer -> orchestrator [on=pass, kind=fan-in]
worker -> orchestrator     [on=blocked, kind=fan-in]
```

## What you get

No matter which worker or reviewer sent the event, mail goes to **`orchestrator.1`**
(the first instance of the target role). That is how every pair’s **pass** and
**blocked** lines converge on one orchestrator.

## One reviewer for all workers

Set `reviewer [count=1]` and fan-in on submit so all workers share one reviewer:

```text
worker -> reviewer [on=submit, kind=fan-in]
reviewer -> worker [on=fail, kind=pair]
```

Submit from any worker lands on **reviewer.1**. Keep **fail** as **pair** so
findings still return to the worker that submitted.

The wteam repo ships this layout as `presets/fan-in.dot`.

## When to choose fan-in

- One orchestrator should see all passes and blocks.
- One shared reviewer (or gate) for the whole team instead of one per pair.

Use [broadcast](./broadcast/) when you need **every** instance to receive a copy,
not just the first.

[Back to mail routing overview](./)

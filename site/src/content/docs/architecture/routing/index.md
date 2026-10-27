---
title: Mail routing
description: Who receives mail when dispatch, submit, pass, fail or blocked happens.
---

Your **team graph** (a Graphviz DOT file, usually `team.dot`) defines who talks to
whom. Each arrow can name:

- **`on=`** — which step triggers the message (`dispatch`, `submit`, `pass`,
  `fail`, `blocked`, or `message`).
- **`kind=`** — which **instances** of the target role receive it.

If you omit `kind`, wteam uses **`pair`**. The other options are **`broadcast`**
and **`fan-in`**.

| Kind | Who gets the mail | Common in default setups |
|---|---|---|
| [pair](./pair/) | The matching pair index (`worker.2` → `reviewer.2`) | Submit and fail between worker and reviewer |
| [broadcast](./broadcast/) | Every instance of that role | Orchestrator dispatch to all workers |
| [fan-in](./fan-in/) | Always instance `.1` of that role | Pass and blocked back to `orchestrator.1` |

Start from the graph in [Concepts — Team graph](../../concepts/#team-graph).
Attribute reference: [Reference — team.dot](../../reference/#team-graph-attributes).

When the built-in graph is not enough, copy a preset from the wteam repo
(`presets/default.dot`, `presets/fan-in.dot`, `presets/security.dot`) and edit
edges there.

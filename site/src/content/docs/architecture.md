---
title: Architecture
description: Module map, turn lifecycle, ticket lifecycle and mailbox sequence.
---

## Module map

The library lives in `src/lib.rs`. `src/main.rs` only parses arguments and prints.

| Module | Role |
|---|---|
| `cli.rs` | Argument parsing and dispatch |
| `commands/` | One file per subcommand group; each returns an `Out` (text and JSON) |
| `engine.rs` | Ticket lifecycle: dispatch, submit, verdict, block |
| `work.rs` | Ticket files, readiness and the work graph |
| `mailbox.rs` | Append-only mail format and parser |
| `state.rs` | `state.json`; written only through `state::with_state` |
| `paths.rs` | Every on-disk path under `.wteam/` |
| `adapters/` | Per-CLI command builders (Claude Code, Codex, opencode, pi, fake) |
| `permissions/` | Profile translation per CLI |
| `team/` | DOT team-graph parser |
| `transport/` | How a delivered mail starts a turn; self-healing and retries in `resume.rs` |
| `watcher.rs` | The `run` loop: orchestrator turns, handoff, budget, stop reasons |
| `backup.rs` | Restore points for files a run applies |
| `worktree.rs` | Isolated worktrees (in progress) |

## Turn lifecycle

```mermaid
sequenceDiagram
  participant Caller as Caller (CLI or engine)
  participant Mail as Mailbox
  participant State as state.json
  participant Turn as wteam __turn
  participant Agent as Agent CLI
  Caller->>Mail: append message
  Caller->>State: lock, read busy
  alt agent idle
    Caller->>State: busy = true
    Caller->>Turn: spawn detached
  else agent busy
    Caller-->>Turn: nothing, the running loop picks mail up
  end
  loop while undelivered mail
    Turn->>State: read mail from offset, advance offset
    Turn->>Agent: run adapter command (role prompt on first turn)
    Agent-->>Turn: output
    Turn->>State: store session_id, context_tokens, turns += 1
  end
  Turn->>State: busy = false
```

On adapter failure (non-zero exit or unparsable output), the turn logs the error,
mails the orchestrator and, if the agent holds a ticket, marks that ticket `Blocked`.

## Ticket lifecycle

```mermaid
sequenceDiagram
  participant O as Orchestrator
  participant W as Worker
  participant R as Reviewer
  participant G as Git
  O->>W: dispatch: brief (ticket, round, prior findings)
  W->>R: submit: summary and diff of ticket files
  alt verdict pass
    R->>G: stage ticket files, commit feat: title (wteam NN)
    R->>O: pass event
  else verdict fail, rounds < max_visits
    R->>W: findings and notes, status Doing
  else verdict fail, rounds >= max_visits
    R->>O: Blocked event
  end
```

## Mailbox delivery

```mermaid
sequenceDiagram
  participant S as Sender
  participant M as mail/<agent>.md
  participant T as Transport
  participant A as Recipient
  S->>M: append marker, headers, body (length bytes)
  S->>T: deliver(agent)
  T->>A: start turn with mail after mail_delivered
  A->>M: wteam inbox reads new messages
```

`mail_delivered` is the byte offset already handed to the agent in a turn
prompt. `mail_read` is what `wteam inbox` already showed.

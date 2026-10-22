---
title: CLI and files
description: Subcommands, team.dot attributes, agent and profile TOML.
---

Every command accepts `--root`, `--slug`, `--agent` and `--json`. Run
`wteam <command> --help` for the full list of flags.

## Commands

| Command | Purpose |
|---|---|
| `init` | Create `.wteam/` and an effort directory |
| `pickup` | Import planning material and verify it (`--from <dir>`) |
| `status` | Tickets, agents and pending events |
| `ready` | Tickets whose blockers are done |
| `dispatch <ticket>` | Assign a ticket to a worker (`--pair <n>`) |
| `submit <ticket>` | Hand a finished round to review (`--summary <text>` or `--summary-file <path>`) |
| `verdict <ticket> pass\|fail` | Record a verdict (`--file <path>` with FINDINGS/NOTES/FOLLOW-UPS sections, or `--finding`, `--note`, `--follow-up`) |
| `block <ticket>` | Mark a ticket blocked (`--reason <text>` or `--reason-file <path>`) |
| `requeue <ticket>` | Return a blocked ticket to todo (`--note <text>` or `--note-file <path>`, not both; `--fresh` also discards the ticket's preserved work: its worktree and branch) |
| `approve <ticket>` | Pass a human gate; the ticket resumes from the gate's pass route |
| `send <to> <message>` | Message another agent |
| `inbox` | Read mail (`--all` for the whole mailbox) |
| `wait` | Block until orchestrator events arrive (`--timeout <secs>`) |
| `run` | Drive the orchestrator until nothing is runnable (see below) |
| `stop` / `restore` | Stop a run, restore backed-up files |
| `report` | Print the stop report |
| `role <name>` | Print a resolved role prompt (project, config dir, then built-in) |
| `graph` | Team or work graph as Mermaid (`--work`, `--team <name>`) |
| `permissions` | Profile translation matrix (`--profile`, `--cli`) |
| `config promote\|list\|show` | Saved teams under `~/.config/wteam`. In progress. |
| `setup` | Detect herdr and pick a transport. In progress. |

## `run` options

| Flag | Default | Meaning |
|---|---|---|
| `--pairs <n>` | 1 | Worker and reviewer pairs (`$pairs` in the team graph) |
| `--team <name\|path>` | `default` | Preset, promoted team, or `.dot` file |
| `--transport <name>` | from config, else `resume` | How a mail starts a turn |
| `--timeout <secs>` | 7200 | Seconds to wait for agent turns before stopping |
| `--max-turns <n>` | 200 | Orchestrator turns, handoff turns included |
| `--dispatch <auto\|model>` | `auto` | `auto`: the watcher dispatches ready tickets and wakes the orchestrator only for blocks and human gates. `model`: the orchestrator dispatches every ticket |
| `--budget-tokens <n>` | none | Stop new work once summed agent context tokens exceed `n` |
| `--replan` | off | Under `auto`: each blocked ticket gets up to two planner turns to rewrite it and requeue it. A ticket blocked after its second re-plan stays blocked |
| `--isolation <none\|worktree>` | `none` | `worktree` is not available yet |

## Team graph attributes

| Attribute | Where | Meaning |
|---|---|---|
| `max_visits` | graph | Review rounds before a ticket is blocked. Default 3. |
| `handoff_tokens` | graph | Context size that triggers an orchestrator handoff. The effective threshold is `min(handoff_tokens, 0.75 × context_window)`. |
| `agent` | node | Agent definition name |
| `count` | node | Instances; `$pairs` takes the `--pairs` value. Default 1. |
| `profile` | node | Overrides the agent's permission profile |
| `prompt` | node | Overrides the agent's role prompt |
| `shape` | node | `box` agent, `parallelogram` command, `hexagon` human gate |
| `script` | command node | Script run from the repo root |
| `timeout` | command node | Seconds before the script counts as failed. Default 600. |
| `on` | edge | Event: `dispatch`, `submit`, `pass`, `fail`, `blocked`, `message` |
| `kind` | edge | `pair`, `broadcast` or `fan-in`. Default `pair`. |

## Agent TOML

Optional. Resolved from `.wteam/agents/<name>.toml`, then
`$WTEAM_CONFIG_DIR/agents/<name>.toml`, then the built-ins.

```toml
name = "worker"
cli = "claude"       # claude | codex | opencode | pi | fake (cursor: in progress)
model = "sonnet"
prompt = "worker"
profile = "edit"
context_window = 200000  # optional; caps the orchestrator handoff at 0.75 of it
extra_args = []      # passed through to the CLI
```

## Profile TOML

Optional. Resolved from `.wteam/profiles/<name>.toml`. Rules use Claude syntax
(see [Permissions](../permissions/)).

```toml
name = "graph"
extends = "edit"                # optional; builtin or another profile
allow = ["Bash(codegraph *)"]   # appended after the parent's lists, duplicates dropped
```

`extends` merges the parent's `allow`, `ask` and `deny` in front of the file's own.
A cycle or an unknown parent is an error that names the chain.

## Project hooks

- `.wteam/hooks/worktree`: `sh .wteam/hooks/worktree <dir>`, cwd the repo root, 120 s
  limit. Runs on each prepared pair worktree before its first turn, and on the repo
  root at `wteam run` start. A failure fails the dispatch (the stdout and stderr tail is the
  error); on the root it is a warning.
- `.wteam/brief.md`: appended verbatim under `## Project notes` in every worker brief
  and reviewer request.

## Files under `.wteam/<slug>/`

`spec.md`, `tickets/NN-name.md`, `state.json`, `mail/<agent>.md`,
`reviews/NN.md`, `queue.md`, `handoffs/`, `logs/<agent>-<turn>.log`,
`backup/` and `worktrees/pair<N>/`. The full layout is in the repository's
`architecture.md`.

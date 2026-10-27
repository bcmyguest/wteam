---
title: Permissions
description: Profile rules and how each CLI translates them.
---

Profiles are Claude permission rules. Each agent names one profile and a `cli`;
see [Providers](../providers/) for install and per-CLI behaviour. wteam
translates it for each CLI. The translation is not always exact, so each cell
is marked:

- ✔ exact: the CLI enforces the rule as written.
- ≈ approximated: the CLI enforces a related rule.
- ✘ not enforced: the CLI cannot enforce the rule.

Run `wteam permissions` to print the current matrix. Filter with
`--profile <name>` and `--cli <name>` (repeat `--cli` to pick columns).

## Rule syntax

Profile TOML holds three lists — `allow`, `ask`, and `deny` — of rule strings.
wteam parses the same forms as Claude Code:

| Form | Example | Applies to |
|---|---|---|
| `Bash(<prefix>)` | `Bash(git status *)` | Shell commands whose invocation matches the prefix ( `*` wildcard ) |
| `Read(<glob>)` | `Read(**/.env)` | Read tool paths |
| `Edit(<glob>)` | `Edit(.wteam/**)` | Edit / Write paths |
| `WebFetch(<spec>)` | `WebFetch(domain:example.com)` | Web fetch domain rules |
| bare tool name | `Read` | Every use of that tool (no parentheses) |

Glob `**` matches any path segment. Custom profiles under `.wteam/profiles/`
may add any parseable rule. The five built-in profiles below ship with wteam.

## Built-in profiles

Five built-in names: `control`, `review`, `edit`, `plan`, `full`. Typical roles:
orchestrator → `control`, worker → `edit`, reviewer → `review`, planner → `plan`.

### `control`

Orchestrator: read the repo and run `wteam`, no edits.

**Allow**

- `Bash(wteam *)`
- `Bash(git status *)`
- `Bash(git log *)`
- `Bash(git diff *)`
- `Read(**)`
- `Bash(cd *)`
- `Bash(ls *)`
- `Bash(cat *)`
- `Bash(head *)`
- `Bash(tail *)`
- `Bash(grep *)`
- `Bash(rg *)`
- `Bash(wc *)`
- `Bash(pwd)`

**Deny**

- `Edit(**)`

### `review`

Reviewer: read code, run checks, write only under `scratch/`.

**Allow**

- `Read(**)`
- `Edit(scratch/**)`
- `Bash(wteam *)`
- `Bash(git diff *)`
- `Bash(git log *)`
- `Bash(cargo test *)`
- `Bash(npm test *)`
- `Bash(pytest *)`
- `Bash(python3 -m pytest *)`
- `Bash(go test *)`
- `Bash(make test)`
- `Bash(make check)`
- `Bash(python3 *)`
- `Bash(python *)`
- `Bash(node *)`
- `Bash(go *)`
- `Bash(cargo *)`
- `Bash(npm *)`
- `Bash(cd *)`
- `Bash(ls *)`
- `Bash(cat *)`
- `Bash(head *)`
- `Bash(tail *)`
- `Bash(grep *)`
- `Bash(rg *)`
- `Bash(wc *)`
- `Bash(pwd)`

**Ask** — (none)

**Deny** — (none; edits outside `scratch/` are blocked by CLI defaults — see below)

### `edit`

Worker: full read/write plus common dev commands.

**Allow**

- `Read(**)`
- `Edit(**)`
- `Bash(wteam *)`
- `Bash(git diff *)`
- `Bash(git status *)`
- `Bash(mkdir *)`
- `Bash(cargo *)`
- `Bash(npm *)`
- `Bash(pytest *)`
- `Bash(python3 -m pytest *)`
- `Bash(go test *)`
- `Bash(make test)`
- `Bash(make check)`
- `Bash(python3 *)`
- `Bash(python *)`
- `Bash(node *)`
- `Bash(go *)`
- `Bash(cd *)`
- `Bash(ls *)`
- `Bash(cat *)`
- `Bash(head *)`
- `Bash(tail *)`
- `Bash(grep *)`
- `Bash(rg *)`
- `Bash(wc *)`
- `Bash(pwd)`

**Ask**

- `Bash(git push *)`

**Deny**

- `Read(**/.env)`
- `Bash(rm -rf *)`

### `plan`

Planner: read code, write `.wteam/` only, run `wteam` and read-only git/shell.

**Allow**

- `Read(**)`
- `Edit(.wteam/**)`
- `Bash(wteam *)`
- `Bash(git status *)`
- `Bash(git log *)`
- `Bash(git diff *)`
- `Bash(ls *)`
- `Bash(cat *)`
- `Bash(head *)`
- `Bash(tail *)`
- `Bash(grep *)`
- `Bash(rg *)`
- `Bash(wc *)`
- `Bash(pwd)`
- `Bash(cd *)`

**Ask** — (none)

**Deny** — (none; see per-CLI notes below)

### `full`

Bypass: no rule lists. Sets each CLI’s unrestricted / bypass mode (`bypass = true`).
Do not use `extends = "full"` on a custom profile.

## How built-ins behave per CLI

`plan` allows no interpreters or build tools (they can write anywhere). It has no
`Edit(**)` deny, because Claude and Cursor resolve deny before allow and that would
shadow the `.wteam` allow. What each CLI enforces for **plan**:

- claude: `Edit`/`Write` outside `.wteam/` are denied (`dontAsk` denies anything not allowed).
- opencode: `edit` outside `.wteam/` is denied (catch-all deny).
- cursor: `Write` is limited to `.wteam/**`.
- codex: any allowed edit switches the whole sandbox to workspace-write (≈). On codex the planner can edit the whole workspace.
- pi: nothing is enforced (✘).

**Review** has no `Edit(**)` deny for the same deny-before-allow reason; scope is
`Edit(scratch/**)` only. Per CLI:

- claude: `Edit`/`Write` outside `scratch/` are denied (`dontAsk`).
- opencode: `edit` outside `scratch/` is denied (catch-all deny).
- cursor: `Write` is limited to `scratch/**`.
- codex: any allowed edit switches the whole sandbox to workspace-write (≈).
- pi: nothing is enforced (✘).

On every CLI, the interpreters and build tools **review** allows (`python3`, `node`,
`go`, `cargo`, `npm`, `pytest`) can still write files through Bash.

The `Bash` allows are not writer-free on every CLI: `git diff --output=<file>` writes a file. Drop `Bash(git diff *)` if that matters for your setup.

## Translation per CLI

`wteam permissions` prints one row per rule. Example rows (full output is longer;
run the command for your build):

```text
control
decision  rule                claude  codex                      opencode  pi                                           cursor
allow     Read(**)            ✔       ≈ sandbox workspace-write  ✔         ✘ pi runs tools unrestricted; use a sandbox  ✔
deny      Edit(**)            ✔       ≈ sandbox workspace-write  ✔         ✘ pi runs tools unrestricted; use a sandbox  ✔

edit
decision  rule               claude  codex                                                            opencode  pi                                           cursor
ask       Bash(git push *)   ✔       ✔                                                                ✔         ✘ pi runs tools unrestricted; use a sandbox  ≈ deny (cursor has no ask)
deny      Read(**/.env)      ✔       ✘ read denies are not enforced; the sandbox can read everything  ✔         ✘ pi runs tools unrestricted; use a sandbox  ✔
```

Default columns: `claude`, `codex`, `opencode`, `pi`, `cursor`. Omit pi with
`--cli claude --cli codex …` if you want a shorter table.

## Why each CLI is lossy

- **Codex** runs agents in a sandbox. Its modes are read-only and workspace-write,
  so read and edit rules become sandbox modes (≈). Codex cannot deny reads of
  specific paths: the sandbox can read everything, so `Read(**/.env)` is ✘.
  `.wteam/` is a writable root, so `wteam` calls can update state.
- **Cursor** has no `ask` decision, so `ask` becomes a deny (≈).
- **Claude Code and opencode** take these rules directly (✔). OpenCode `full`
  adds `--auto`; other profiles set `OPENCODE_PERMISSION` JSON in the environment.
- **Pi** runs tools unrestricted, so every rule is ✘ except `full` bypass metadata.
  Run pi agents inside a sandbox. Pi is **experimental**; wteam does not map profile
  rules to Pi flags.

The `full` profile sets bypass on every CLI (OpenCode: `--auto`; Pi: no extra
flags, still unrestricted). It enforces nothing beyond what each CLI does by default.

## Extending a profile

A project profile can start from another with `extends`. Its rule lists go after the
parent's, duplicates dropped, so a project adds rules without copying the builtin:

```toml
name = "graph"
extends = "edit"
allow = ["Bash(codegraph *)"]
```

A profile that sets `extends` may not set `bypass`, and may not extend a bypass
profile such as `full`. Either is an error naming the profile. Bypass ignores every
rule, so the denies in the chain would vanish silently. A profile without `extends`
may still set `bypass = true`.

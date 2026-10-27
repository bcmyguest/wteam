---
title: Permissions
description: Profile rules and how each CLI translates them.
---

Profiles are Claude permission rules. Each agent names one profile and a `cli`;
see [Providers](./providers/) for install and per-CLI behaviour. wteam
translates it for each CLI. The translation is not always exact, so each cell
is marked:

- ✔ exact: the CLI enforces the rule as written.
- ≈ approximated: the CLI enforces a related rule.
- ✘ not enforced: the CLI cannot enforce the rule.

Run `wteam permissions` to print the current matrix. Filter with
`--profile <name>` and `--cli <name>`.

## Built-in profiles

`control`, `review`, `plan` and `edit` also allow the read-only shell basics `cd`, `ls`,
`cat`, `head`, `tail`, `grep`, `rg`, `wc` and `pwd`.

| Profile | Allow | Ask | Deny |
|---|---|---|---|
| `control` | `Bash(wteam *)`, `Bash(git status *)`, `Bash(git log *)`, `Bash(git diff *)`, `Read(**)` | | `Edit(**)` |
| `review` | `Read(**)`, `Edit(scratch/**)`, `Bash(wteam *)`, `Bash(git diff *)`, `Bash(git log *)`, toolchains (`python3 *`, `python *`, `node *`, `go *`, `cargo *`, `npm *`, `pytest *`), test runners (`cargo test`, `npm test`, `python3 -m pytest`, `go test`, `make test`, `make check`) | | |
| `edit` | `Read(**)`, `Edit(**)`, `Bash(wteam *)`, `Bash(git diff *)`, `Bash(git status *)`, `Bash(mkdir *)`, `Bash(python3 *)`, `Bash(python *)`, `Bash(node *)`, `Bash(go *)`, `Bash(cargo *)`, `Bash(npm *)`, `Bash(pytest *)`, the test runners above | `Bash(git push *)` | `Read(**/.env)`, `Bash(rm -rf *)` |
| `plan` | `Read(**)`, `Edit(.wteam/**)`, `Bash(wteam *)`, `Bash(git status *)`, `Bash(git log *)`, `Bash(git diff *)`, `Bash(ls *)`, `Bash(cat *)`, `Bash(head *)`, `Bash(tail *)`, `Bash(grep *)`, `Bash(rg *)`, `Bash(wc *)`, `Bash(pwd)`, `Bash(cd *)` | | |
| `full` | bypass all permissions | | |

`plan` is for the planner: it reads code, writes `.wteam/` and runs `wteam`. It allows no interpreters or build tools, since those can write anywhere. It has no `Edit(**)` deny, because claude and cursor resolve deny before allow and that would shadow the `.wteam` allow. What each CLI enforces:

- claude: `Edit`/`Write` outside `.wteam/` are denied (`dontAsk` denies anything not allowed).
- opencode: `edit` outside `.wteam/` is denied (catch-all deny).
- cursor: `Write` is limited to `.wteam/**`.
- codex: any allowed edit switches the whole sandbox to workspace-write (≈). On codex the planner can edit the whole workspace.
- pi: nothing is enforced (✘).

`review` is for the reviewer: it reads code, runs checks and writes only under `scratch/`. What each CLI enforces:

- claude: `Edit`/`Write` outside `scratch/` are denied (`dontAsk` denies anything not allowed).
- opencode: `edit` outside `scratch/` is denied (catch-all deny).
- cursor: `Write` is limited to `scratch/**`.
- codex: any allowed edit switches the whole sandbox to workspace-write (≈). On codex the reviewer can edit the whole workspace.
- pi: nothing is enforced (✘).

On every CLI, the interpreters and build tools `review` allows (`python3`, `node`, `go`, `cargo`, `npm`, `pytest`) can still write files through Bash.

The `Bash` allows are not writer-free on every CLI: `git diff --output=<file>` writes a file. Drop `Bash(git diff *)` if that matters for your setup.

## Translation per CLI

Output of `wteam permissions` (abridged, as of this build; `pi` is a fifth column
that enforces nothing, so every row is ✘):

```text
control
decision  rule                claude  codex                      opencode  cursor
allow     Read(**)            ✔       ≈ sandbox workspace-write  ✔         ✔
deny      Edit(**)            ✔       ≈ sandbox workspace-write  ✔         ✔

edit
decision  rule               claude  codex                                                            opencode  cursor
ask       Bash(git push *)   ✔       ✔                                                                ✔         ≈ deny (cursor has no ask)
deny      Read(**/.env)      ✔       ✘ read denies are not enforced; the sandbox can read everything  ✔         ✔
```

## Why each CLI is lossy

- **Codex** runs agents in a sandbox. Its modes are read-only and workspace-write,
  so read and edit rules become sandbox modes (≈). Codex cannot deny reads of
  specific paths: the sandbox can read everything, so `Read(**/.env)` is ✘.
  `.wteam/` is a writable root, so `wteam` calls can update state.
- **Cursor** has no `ask` decision, so `ask` becomes a deny (≈).
- **Claude Code and opencode** take these rules directly (✔). OpenCode `full`
  adds `--auto`; other profiles set `OPENCODE_PERMISSION` JSON in the environment.
- **pi** runs tools unrestricted, so every rule is ✘. Run pi agents inside a sandbox.
  Pi is **experimental**; wteam does not map profile rules to Pi flags.

The `full` profile sets bypass on every CLI (OpenCode: `--auto`; Pi: no extra
flags, still unrestricted). It enforces nothing beyond what each CLI does by default.

Run `wteam permissions --cli pi` to print the Pi column; the default table shows
claude, codex, opencode and cursor only.

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

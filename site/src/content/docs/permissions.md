---
title: Permissions
description: Profile rules and how each CLI translates them.
---

Profiles are Claude permission rules. Each agent names one profile. wteam
translates it for each CLI. The translation is not always exact, so each cell
is marked:

- ✔ exact: the CLI enforces the rule as written.
- ≈ approximated: the CLI enforces a related rule.
- ✘ not enforced: the CLI cannot enforce the rule.

Run `wteam permissions` to print the current matrix. Filter with
`--profile <name>` and `--cli <name>`.

## Built-in profiles

`control`, `review` and `edit` also allow the read-only shell basics `cd`, `ls`,
`cat`, `head`, `tail`, `grep`, `rg`, `wc` and `pwd`.

| Profile | Allow | Ask | Deny |
|---|---|---|---|
| `control` | `Bash(wteam *)`, `Bash(git status *)`, `Bash(git log *)`, `Bash(git diff *)`, `Read(**)` | | `Edit(**)` |
| `review` | `Read(**)`, `Bash(wteam *)`, `Bash(git diff *)`, `Bash(git log *)`, toolchains (`python3 *`, `python *`, `node *`, `go *`, `cargo *`, `npm *`, `pytest *`), test runners (`cargo test`, `npm test`, `python3 -m pytest`, `go test`, `make test`, `make check`) | | `Edit(**)` |
| `edit` | `Read(**)`, `Edit(**)`, `Bash(wteam *)`, `Bash(git diff *)`, `Bash(git status *)`, `Bash(mkdir *)`, `Bash(python3 *)`, `Bash(python *)`, `Bash(node *)`, `Bash(go *)`, `Bash(cargo *)`, `Bash(npm *)`, `Bash(pytest *)`, the test runners above | `Bash(git push *)` | `Read(**/.env)`, `Bash(rm -rf *)` |
| `full` | bypass all permissions | | |

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
- **Claude Code and opencode** take these rules directly (✔).
- **pi** runs tools unrestricted, so every rule is ✘. Run pi agents inside a sandbox.

The `full` profile sets bypass on every CLI and enforces nothing.

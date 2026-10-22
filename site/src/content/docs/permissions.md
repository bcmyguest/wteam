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

| Profile | Allow | Ask | Deny |
|---|---|---|---|
| `control` | `Bash(wteam *)`, `Bash(git status *)`, `Bash(git log *)`, `Bash(git diff *)`, `Read(**)` | | `Edit(**)` |
| `review` | `Read(**)`, `Bash(wteam *)`, `Bash(git diff *)`, `Bash(git log *)`, `Bash(cargo test *)`, `Bash(npm test *)`, `Bash(pytest *)` | | `Edit(**)` |
| `edit` | `Read(**)`, `Edit(**)`, `Bash(wteam *)`, `Bash(git diff *)`, `Bash(git status *)`, `Bash(cargo *)`, `Bash(npm *)`, `Bash(pytest *)` | `Bash(git push *)` | `Read(**/.env)`, `Bash(rm -rf *)` |
| `full` | bypass all permissions | | |

## Translation per CLI

Output of `wteam permissions` (abridged, as of this build):

```text
control
decision  rule                claude  codex                opencode  cursor
allow     Read(**)            ✔       ≈ sandbox read-only  ✔         ✔
deny      Edit(**)            ✔       ≈ sandbox read-only  ✔         ✔

edit
decision  rule               claude  codex                                   opencode  cursor
deny      Read(**/.env)      ✔       ✘ read denies are not enforced          ✔         ✔
ask       Bash(git push *)   ✔       ✔                                       ✔         ≈ deny (cursor has no ask)
```

## Why each CLI is lossy

- **Codex** runs agents in a sandbox. Its modes are read-only and workspace-write,
  so read and edit rules become sandbox modes (≈). Codex cannot deny reads of
  specific paths: the sandbox can read everything, so `Read(**/.env)` is ✘.
- **Cursor** has no `ask` decision, so `ask` becomes a deny (≈).
- **Claude Code and opencode** take these rules directly (✔).

The `full` profile sets bypass on every CLI and enforces nothing.

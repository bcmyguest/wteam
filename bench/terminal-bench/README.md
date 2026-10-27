# Terminal-Bench 4.0 (Harbor)

[Terminal-Bench 4.0](https://www.tbench.ai/) is the current **agent-level** terminal benchmark (**66 tasks** on [Harbor Hub](https://hub.harborframework.com/datasets) as `terminal-bench/terminal-bench`). Scores in the **~mid‑60%** range for top agents (Oct 2026) — unlike saturated Commit0 / Verified suites.

This folder wraps **[Harbor](https://github.com/harbor-framework/harbor)** for wteam experiments. Mapping to [`bench/runner/`](../runner/) modes:

| wteam runner mode | TB 4.0 here |
|-------------------|-------------|
| **continuous** | `run_continuous.sh` — one `cursor-cli` (or `--agent`) episode per task |
| **fresh** | `run_fresh.sh` — set `TB_FRESH_ATTEMPTS=3` for multiple independent attempts per task |
| **team** | Not wired — needs a custom Harbor agent around wteam (`run_team.sh` exits 2) |

## Prerequisites

```bash
uv tool install harbor
docker info   # local runs
command -v agent   # for cursor-cli agent
```

Host benches ([`bench/runner/`](../runner/), brownfield) use **`agent` on the host** — normal **`agent login`** is enough.

Harbor’s **`cursor-cli`** runs **inside Docker**. By default we **bind-mount** `~/.config/cursor/auth.json` (`TB_MOUNT_CURSOR_AUTH=auto`, override with `TB_CURSOR_AUTH_FILE`). That matches how you’re already logged in and avoids putting OAuth in env vars.

**Caveat (upstream Harbor):** the stock **`cursor-cli`** agent still expects **`CURSOR_API_KEY`** inside the trial even with the mount. Until Harbor or a custom wteam agent reads the mounted file, set an API key in `.env.local` only as a fallback. Longer term: custom Harbor agent (auth mount only) or `acp:cursor` with `authenticate_method_id=cursor_login`.

## Quick start

Prefer **[`bench/spin-up/run.sh`](../spin-up/run.sh)** — one command, OAuth bind, log file:

```bash
sh bench/spin-up/run.sh tb-smoke 1
sh bench/spin-up/run.sh tb-cursor 5
```

Manual:

```bash
# 1) Oracle smoke (1 task, no model spend)
sh bench/terminal-bench/run_smoke.sh 1

# 2) Cursor on first 5 tasks (login-first: acp:cursor via spin-up, or cursor-cli + optional API key)
export TB_MODEL=cursor/composer-2.5
export TB_AGENT=acp:cursor
sh bench/terminal-bench/run_continuous.sh 5

# 3) One named task
TB_INCLUDE_TASK='hello-world' sh bench/terminal-bench/run_continuous.sh 1

# 4) Mini matrix (smoke + 3 cursor tasks)
sh bench/terminal-bench/run_matrix.sh 3
```

Results land under `bench/terminal-bench/jobs/` (or `TB_JOBS_DIR`). Inspect with Harbor’s job UI / logs in each job directory.

## Env

| Var | Default |
|-----|---------|
| `TB_DATASET` | `terminal-bench/terminal-bench` |
| `TB_AGENT` | `cursor-cli` |
| `TB_MODEL` | `BENCH_MODEL` or `composer-2.5` |
| `TB_JOBS_DIR` | `./jobs` |
| `TB_ENV` | unset (local Docker); set `modal` / `daytona` + API keys for cloud |
| `TB_N_CONCURRENT` | `1` |
| `TB_INCLUDE_TASK` | `-i` filter for one task name |
| `TB_MOUNT_CURSOR_AUTH` | `auto` (default): bind-mount OAuth `auth.json` when present |
| `TB_CURSOR_AUTH_FILE` | Host path to mount (default `~/.config/cursor/auth.json`) |
| `TB_SKIP_CREDENTIAL_CHECKS` | `auto` (default): `--skip-credential-checks` when `cursor-cli` and no `CURSOR_API_KEY` |

Direct Harbor equivalent:

```bash
harbor run -d terminal-bench/terminal-bench -a cursor-cli -m composer-2.5 -l 5 -k 1
```

## Other benchmarks (deferred)

Alternatives recorded in project memory (`coding-agent::wteam`): SWE-bench Pro V2, SWE-EVO, SWE-bench Multilingual, internal brownfield. See conversation / `memory_recall` for the Oct 2026 saturation notes.

## Next steps

- Custom Harbor agent: wteam orchestrator inside the sandbox for **team** arm.
- Add TB matrix to `bench/run_full_matrix.sh` behind `RUN_TB=1`.
- Publish a small fixed task slice (e.g. 10 tasks) for CI smoke.

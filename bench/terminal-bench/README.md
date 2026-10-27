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

Cursor auth must work for `agent` / Harbor’s `cursor-cli` agent (same as other bench arms).

## Quick start

```bash
# 1) Oracle smoke (1 task, no model spend)
sh bench/terminal-bench/run_smoke.sh 1

# 2) Cursor on first 5 tasks
export TB_MODEL=composer-2.5   # or harbor’s model string for cursor-cli
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

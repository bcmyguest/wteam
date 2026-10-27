# Configurable bench runner

Commit0-style tasks (hidden pytest grade) with **any wteam CLI** and three **context** arms on the **same agent config**:

| Mode | Context | What it measures |
|------|---------|------------------|
| **continuous** | One session, full goal | Single long chat (optional `PLAN=` in that same session) |
| **fresh** | New session per ticket | Same model/cli, no resume between tickets |
| **team** | wteam pairs | Same layout as [`bench/commit0/run_wteam.sh`](../commit0/run_wteam.sh): planner → tickets → dispatch |

[`bench/commit0/`](../commit0/) remains the published Claude Code harness; this tree is CLI-agnostic.
Default CLI today is **cursor** (`agent`); set `BENCH_CLI=claude` for Haiku/Sonnet parity.

## Configuration

| Env | Meaning |
|-----|---------|
| `BENCH_CLI` | `claude`, `cursor`, `codex`, … (wteam adapter name) |
| `BENCH_MODEL` | Default model for all roles |
| `BENCH_PLANNER_MODEL` / `BENCH_WORKER_MODEL` / `BENCH_REVIEWER_MODEL` | Team overrides (like commit0’s three model args) |
| `BENCH_REVIEW` | `autopass` (default, fake reviewer) or `on`/`real` (same CLI as workers) |
| `BENCH_PAIRS` | Team pair count (default 4) |
| `WTEAM_BENCH_ISOLATION` | Team isolation (default `worktree`) |
| `CODEGRAPH`, `SHAPE=coupled`, `OVERLAP`, `RUN_TIMEOUT` | Same as commit0 wteam |
| `PLAN=<dir>` | Saved `spec.md` + `NN-*.md` — continuous loads all in one prompt; fresh runs one session per ticket |

Team invocation matching commit0:

```bash
export BENCH_CLI=claude
sh bench/runner/run_team.sh tinydb /tmp/t-team haiku haiku sonnet 4
```

Cursor (one model for every role):

```bash
export BENCH_CLI=cursor BENCH_MODEL=composer-2.5 BENCH_REVIEW=autopass
sh bench/runner/run.sh fresh incidentkit-debug /tmp/ikd-fresh
```

## Commands

```bash
sh bench/runner/run.sh continuous <lib> <dir> [model]
sh bench/runner/run.sh fresh       <lib> <dir> [model]
sh bench/runner/run.sh team        <lib> <dir> <pairs> [model]
# or team with three models:
sh bench/runner/run_team.sh <lib> <dir> <planner> <worker> <reviewer> <pairs>

BENCH_LIBS=tinydb sh bench/runner/run_matrix.sh /tmp/matrix-three-way
```

Metrics: `<dir>.logs/metrics.txt` (`bench_mode`, `bench_cli`, grade `RESULT`, tokens or `cost_usd`).

[`bench/agent/`](../agent/) scripts delegate here for backward compatibility.

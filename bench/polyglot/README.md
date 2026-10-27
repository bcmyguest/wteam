# Aider polyglot benchmark (Python)

34 Exercism-style exercises from [Aider's polyglot benchmark](https://github.com/Aider-AI/polyglot-benchmark). `grade.sh` always runs **pristine** tests from your checkout.

## Agent CLI (via `bench/runner`)

Same env as [commit0 runner](../runner/README.md): `BENCH_CLI`, `BENCH_MODEL`, `BENCH_REVIEW`, etc. Default CLI is **cursor** (`agent`).

```bash
POLYGLOT=/path/to/polyglot-benchmark
export BENCH_CLI=cursor BENCH_MODEL=composer-2.5

# One long session (baseline)
HIDDEN=1 sh bench/polyglot/run_single.sh "$POLYGLOT" /tmp/pg-single

# wteam, 4 pairs, hidden tests
HIDDEN=1 sh bench/polyglot/run_wteam.sh "$POLYGLOT" /tmp/pg-team 4

# Smoke: first 3 exercises only
BENCH_LIMIT=3 sh bench/polyglot/run_single.sh "$POLYGLOT" /tmp/pg-smoke

# Three-way matrix (continuous / fresh / team)
HIDDEN=1 sh bench/polyglot/run_matrix.sh "$POLYGLOT" /tmp/pg-matrix 4
```

Metrics and `RESULT passed/total` land in `<work-dir>.logs/metrics.txt`.

Claude parity (historical tables in `bench/results/`):

```bash
export BENCH_CLI=claude BENCH_MODEL=haiku
HIDDEN=1 sh bench/polyglot/run_wteam.sh "$POLYGLOT" /tmp/pg-haiku 4
```

## Modes

| Env | Effect |
|-----|--------|
| `HIDDEN=1` | Remove unit tests from the repo before agents start (Aider protocol) |
| `BENCH_LIMIT=N` | First N exercises only |
| `DISPATCH=auto\|model` | Team orchestrator dispatch (default watcher auto) |

Legacy scripts `run_single.sh` / `run_wteam.sh` delegate to `bench/runner/run_polyglot_*.sh`.

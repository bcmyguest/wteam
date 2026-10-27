# Agent bench (thin wrappers)

Scripts here default `BENCH_CLI=cursor` and call [`bench/runner/`](../runner/) for the real harness.

Use the runner directly when you want another CLI or the three-way matrix:

```bash
# Same Composer config: continuous vs fresh-per-ticket vs wteam
export BENCH_CLI=cursor BENCH_MODEL=composer-2.5
sh bench/runner/run_matrix.sh /tmp/matrix

# Claude parity with commit0 team layout
export BENCH_CLI=claude BENCH_REVIEW=on
sh bench/runner/run_team.sh tinydb /tmp/t haiku haiku sonnet 4
```

See [`bench/runner/README.md`](../runner/README.md) for modes and env vars.

Legacy names:

| Script | Runner mode |
|--------|-------------|
| `run_single.sh` | `continuous` |
| `run_fresh.sh` | `fresh` |
| `run_wteam.sh` | `team` |

Brownfield Rust task: [`bench/brownfield/`](../brownfield/) (separate task; can set `BENCH_CLI` when wired).

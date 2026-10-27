# Spin-up runner (wteam harness)

One command to exercise **wteam’s bench harness** — continuous vs fresh vs **team** (tickets, dispatch, pairs), not “how good is Cursor on TB.”

The CLI (`BENCH_CLI`, default `cursor`) is only the **driver**; what you’re comparing is **context and orchestration** on the same hidden grade.

Uses [`../run-isolated.sh`](../run-isolated.sh) for clean logs under `/tmp/wteam-spin-*.log` (or `RUN_LOG`).

## Harness (what you want)

```bash
# Primary: brownfield three-way matrix (hard task; structure vs single session)
sh bench/spin-up/run.sh harness
# same as:
sh bench/spin-up/run.sh harness-brownfield

# Commit0 tinydb three-way (smoke / saturated; cheap discriminator check)
sh bench/spin-up/run.sh harness-commit0

# Both matrices back-to-back
sh bench/spin-up/run.sh harness-all

# Single arms (quick)
sh bench/spin-up/run.sh harness-brownfield-team
sh bench/spin-up/run.sh harness-brownfield-continuous
```

Env: `BENCH_CLI`, `BENCH_MODEL`, `BENCH_REVIEW`, `PAIRS`, `WTEAM`, `MATRIX_BASE`, `BF_RUN_BASE`.

Implementation: [`../runner/`](../runner/) (Commit0-style libs), [`../brownfield/`](../brownfield/) (Rust gate on pinned tree).

### Terminal-Bench question set (`harness-tb`)

Same **continuous / fresh / team** arms, but tasks and grading come from **TB 4.0** (Harbor verifier), not pytest hidden gates:

```bash
sh bench/spin-up/run.sh harness-tb 3      # matrix on first 3 tasks
sh bench/spin-up/run.sh harness-tb-team 1 # team arm smoke
```

See [`../terminal-bench/harness/`](../terminal-bench/harness/).

## External (not the harness)

Terminal-Bench via Harbor tests **agent-in-Docker on terminal tasks**, not wteam `run` / tickets / team. TB **team** arm is still unwired.

```bash
sh bench/spin-up/run.sh external-tb-smoke 1   # Harbor/Docker/dataset only
sh bench/spin-up/run.sh external-tb-cursor 5  # optional; leaderboard-shaped
```

See [`../terminal-bench/`](../terminal-bench/) and [`../BENCH_OPTIONS.md`](../BENCH_OPTIONS.md).

## Overnight (resumable)

Partial failures are expected — phases leave markers and `SUMMARY.md` even when arms fail.

```bash
sh bench/spin-up/overnight.sh
# morning (or anytime):
cat /tmp/wteam-overnight-latest/SUMMARY.md
# after crash / usage limit / failed phase — skips .phase_done arms:
sh bench/spin-up/resume-overnight.sh
```

Markers under the overnight base / nested matrices: `*.phase_done`, `*.phase_failed`, `*.phase_running`.
Brownfield arms always write `metrics.txt` + grade even if the agent exits non-zero (e.g. usage limit).

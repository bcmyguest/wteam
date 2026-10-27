# harness-tb — Terminal-Bench tasks, wteam harness arms

**Question set:** [Terminal-Bench 4.0](https://www.tbench.ai/) (`terminal-bench/terminal-bench`, 66 tasks).
**Grading:** Harbor verifier (reward 0/1), same as the public leaderboard.
**What varies:** our **context arms** (same idea as [`bench/runner/`](../../runner/)), not “which Cursor build wins TB.”

| Arm | Meaning on TB |
|-----|----------------|
| **continuous** | One agent episode per task (single session on the full `instruction.md`) |
| **fresh** | Multiple independent attempts per task (`-k N` trials) |
| **team** | **wteam** inside the task container: init → tickets → `wteam run` on the TB goal |

Driver CLI defaults to Cursor `agent` (`BENCH_CLI=cursor`); swap with `BENCH_CLI` / adapter env like other benches.

## Prerequisites

- `harbor`, Docker, release **`wteam`** on the host (`cargo build --release`)
- Host Cursor **agent install dir** (not just the `~/.local/bin/agent` symlink) — we mount the
  resolved `…/versions/<ver>/` tree so `node` + `index.js` exist in the container. Mounting
  only the wrapper causes ~100ms “agent” exits and all-zero rewards.
- `~/.config/cursor/auth.json` from `agent login` (bind-mounted read-only)

## Run

```bash
# One task, team arm (smoke)
sh bench/terminal-bench/harness/run_team.sh 1

# Three-way matrix on first N tasks (sequential)
sh bench/terminal-bench/harness/run_matrix.sh 3

# Or via spin-up
sh bench/spin-up/run.sh harness-tb 3
```

Logs: `TB_HARNESS_DIR` or `/tmp/wteam-harness-tb-<timestamp>/`.

## How it works

Harbor runs each task’s **Docker environment** and **verifier**. The agent phase uses a custom Harbor agent ([`wteam_harbor_agent.py`](wteam_harbor_agent.py)) instead of stock `cursor-cli`:

- Mounts host `wteam`, `agent`, OAuth `auth.json`, and [`in_container_*.sh`](in_container_continuous.sh) into the sandbox.
- **continuous** → [`in_container_continuous.sh`](in_container_continuous.sh) (one `agent -p` with the TB instruction).
- **team** → [`in_container_team.sh`](in_container_team.sh) (`wteam init`, one orchestrated `wteam run` with the TB instruction as the bench goal).

**Not yet:** planner-quality ticket splits for TB, fresh-as-separate-sessions per sub-step, or parallel matrix. **Team** is a first slice (single `wteam run` per task, not Harbor’s unwired `run_team.sh` stub).

## vs `external-tb-*`

| | harness-tb | external-tb-cursor |
|--|------------|-------------------|
| Agent | Custom **wteam-harness** (+ mounts) | Stock Harbor **cursor-cli** / ACP |
| Team arm | **wteam run** in container | Not available |
| Purpose | Orchestration on TB tasks | Leaderboard-shaped agent baseline |

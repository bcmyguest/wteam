# harness-tb — Terminal-Bench tasks, wteam harness arms

**Question set:** [Terminal-Bench 4.0](https://hub.harborframework.com/datasets/terminal-bench/terminal-bench/4) (`terminal-bench/terminal-bench@4`, 66 tasks).
**Grading:** Harbor verifier (reward 0/1), same as the public leaderboard.
**What varies:** our **context arms** (same idea as [`bench/runner/`](../../runner/)), not “which Cursor build wins TB.”

| Arm | Meaning on TB |
|-----|----------------|
| **continuous** | One agent episode per task (single session on the full `instruction.md`) |
| **fresh** | Multiple independent attempts per task (`-k N` trials) |
| **team** | **wteam** inside the task container: init → tickets → `wteam run` on the TB goal |

Driver CLI defaults to Cursor `agent` (`BENCH_CLI=cursor`); set `BENCH_CLI=codex` or `BENCH_CLI=opencode` (local Lemonade) like other benches.

## Prerequisites

- `harbor`, Docker, a static musl **`wteam`** on the host (`rustup target add x86_64-unknown-linux-musl && cargo build --release --target x86_64-unknown-linux-musl`). Task images can have older glibc than the host; the harness prefers this binary when present.
- Host Cursor **agent install dir** (not just the `~/.local/bin/agent` symlink) — we mount the
  resolved `…/versions/<ver>/` tree so `node` + `index.js` exist in the container. Mounting
  only the wrapper causes ~100ms “agent” exits and all-zero rewards.
- `~/.config/cursor/auth.json` from `agent login` (bind-mounted read-only)
- For Codex: host `codex`, its sibling `codex-code-mode-host`, and `~/.codex/auth.json`

## Run

```bash
# One task, team arm (smoke)
sh bench/terminal-bench/harness/run_team.sh 1

# Matched 10-task Codex Luna comparison, one attempt per task
export BENCH_CLI=codex BENCH_MODEL=gpt-6-luna TB_MODEL=gpt-6-luna
export TB_DATASET=terminal-bench/terminal-bench@4 TB_N_CONCURRENT=2
export TB_PAIRS=4 BENCH_REVIEW=autopass
TB_HARNESS_DIR=/tmp/wteam-tb-luna-single sh bench/terminal-bench/harness/run_continuous.sh 10
TB_HARNESS_DIR=/tmp/wteam-tb-luna-team sh bench/terminal-bench/harness/run_team.sh 10

# OpenCode + local Lemonade (wteam harness continuous)
TB_MODE=harness sh bench/terminal-bench/run_opencode_lemonade.sh 1 Nemotron-3.5-Bolt-E4B-GGUF

# Three-way matrix on first N tasks (sequential)
sh bench/terminal-bench/harness/run_matrix.sh 3

# Or via spin-up
sh bench/spin-up/run.sh harness-tb 3
```

Logs: `TB_HARNESS_DIR` or `/tmp/wteam-harness-tb-<timestamp>/`.

## How it works

Harbor runs each task’s **Docker environment** and **verifier**. The agent phase uses a custom Harbor agent ([`wteam_harbor_agent.py`](wteam_harbor_agent.py)) instead of stock `cursor-cli`:

- Mounts host `wteam`, the selected CLI, its auth file, and [`in_container_*.sh`](in_container_continuous.sh) into the sandbox.
- **continuous** → [`in_container_continuous.sh`](in_container_continuous.sh) (one CLI episode with the TB instruction).
- **team** → [`in_container_team.sh`](in_container_team.sh) (one planning episode, ticket validation, and an orchestrated `wteam run` with four pairs by default). The task container installs Git if needed. The team run budget defaults to the Harbor task budget of 28,800 seconds; reviews autopass by default.

The task's hidden verifier runs after the agent exits. The custom agent raises an error when setup or the agent command exits nonzero, or when the agent exit marker is missing. Inspect `agent/exit_code.txt` and the per-trial logs before reporting a score. The team arm also saves its wteam logs and state under the trial's `agent/` directory.

## vs `external-tb-*`

| | harness-tb | external-tb-cursor |
|--|------------|-------------------|
| Agent | Custom **wteam-harness** (+ mounts) | Stock Harbor **cursor-cli** / ACP |
| Team arm | **wteam run** in container | Not available |
| Purpose | Orchestration on TB tasks | Leaderboard-shaped agent baseline |

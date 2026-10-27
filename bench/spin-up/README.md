# Spin-up runner

One command for the runs we repeat — no manual Harbor flags, auth mounts, or nested shell debugging.

Uses [`../run-isolated.sh`](../run-isolated.sh) so output stays readable in agent sessions.

```bash
# Harbor + dataset OK (no model spend)
sh bench/spin-up/run.sh tb-smoke 1

# Terminal-Bench slice with your existing Cursor login (ACP + OAuth bind, not API-key-first)
sh bench/spin-up/run.sh tb-cursor 5

# Smoke + 3 cursor tasks
sh bench/spin-up/run.sh matrix-cursor 3

# Brownfield team arm (builds release wteam if needed)
sh bench/spin-up/run.sh brownfield-team

# Commit0 tinydb continuous / fresh / team
sh bench/spin-up/run.sh runner-matrix
```

Logs go to `/tmp/wteam-spin-*.log` unless you set `RUN_LOG`.

**tb-cursor** sets `TB_AGENT=acp:cursor` and `authenticate_method_id=cursor_login` (see [`../terminal-bench/common.sh`](../terminal-bench/common.sh)). Host benches still use plain `agent` on the host via [`../runner/`](../runner/).

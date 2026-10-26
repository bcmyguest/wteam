# Brownfield grader adaptations

Historical tests are pinned at commit `e8d709d` (see `grade.sh`). The candidate
repo is archived from `ba759a4` via `prepare.sh`. Both commits bound the same
task: bulk `dispatch --fill` and watcher liveness.

The upstream suite at `e8d709d` asserts **exact substrings** in `roles/orchestrator.md`
and in orchestrator kickoff/digest prompts. Implementations that satisfy the task
often rephrase those lines; failing on wording would measure prompt copying, not
behavior.

`apply_adaptations.py` patches only the extracted test tree before `cargo test`:

| Historical test | Adaptation |
|-----------------|------------|
| `orchestrator_runs_one_command_per_shell_call_and_fills_pairs` | Same requirements (no shell chaining, denied-command handling, `--fill` documented, no chained dispatches) checked by meaning, not one fixed sentence. |
| `kickoff_and_digest_end_with_the_fill_hint` | Still requires at least two `dispatch --fill` mentions in logged prompts **and** language that idle pairs should be filled / kept busy. |
| `one_dispatch_per_turn_runs_every_ticket_with_four_pairs` | **Unchanged** — full behavioral check at four pairs. |

`cargo fmt --check` runs in `grade.sh` before tests so formatting is part of the
hidden gate (also named in `task.md` and validate-out).

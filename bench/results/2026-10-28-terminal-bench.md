# Terminal-Bench 4.0: Codex Luna ten-task attempt (2026-10-28)

These attempts did **not** produce a comparable single-versus-team result. The team job selected ten tasks from `terminal-bench/terminal-bench@4` with `-l 10`, `BENCH_CLI=codex`, `BENCH_MODEL=gpt-6-luna`, four pairs, autopass review, one attempt per task, and two concurrent Harbor trials. Its raw Harbor result was 0/10 in 17m11s. Agent and harness failures below make the raw zero unsuitable as a wteam performance score.

The job and per-trial logs are under `/tmp/wteam-tb-luna-team10-valid-20261028/jobs/tb-harness-team-20261027-234656/` on the benchmark host. The original task set has 66 tasks; this run used the first ten selected by Harbor.

| Task | Harbor reward | Observed interruption |
| --- | ---: | --- |
| `layout-config-recreation2` | 0 | Agent exit marker and wteam state were not captured. |
| `photonic-waveguide-routing` | 0 | Agent exit marker and wteam state were not captured. |
| `mp-checkpoint-consolidation` | 0 | Planner hit the Codex usage limit. |
| `biped-contact-dynamics` | 0 | Worker ticket blocked after the Codex usage limit. |
| `ks-solver-cpp` | 0 | Codex planner failed with `workspace routing discovery failed`. |
| `react-lead-form` | 0 | Codex planner failed with `workspace routing discovery failed`. |
| `lake-temp-glm` | 0 | Planner hit the Codex usage limit. |
| `takens-embedding-lean` | 0 | Agent script stopped before planning because this task uses `/task`, while the harness assumed `/app`. |
| `satb-audio-transcription` | 0 | Planner hit the Codex usage limit. |
| `formal-crypto` | 0 | Planner hit the Codex usage limit. |

Before this job, two diagnostic team attempts also stopped early. The first used a host glibc-linked `wteam` binary, which could not start in an older task image. The second used a static binary but pointed the fake reviewer at a missing role prompt. Their raw zeros are excluded from this table. The job above used a static musl binary, an existing reviewer role, and ticket checks normalized to `true` so Harbor's hidden verifier grades the artifact. A later harness change maps `/app` to `/task` when needed. A successful rerun is required before a team score can be reported.

## Single-agent arm: interrupted

The matching Codex Luna single-agent job started with the same ten-task selection, one attempt per task, and two concurrent trials. Harbor stopped repeatedly with exit codes 137 and 143, and the job was later renamed `tb-harness-continuous-20261028-081920.PAUSED-for-reward-smoke` by another process. No final single-agent score exists. Its preserved record contains five completed trials, two interrupted trials, and three pending trials:

| Task | Harbor reward | Agent exit | Observation |
| --- | ---: | ---: | --- |
| `layout-config-recreation2` | 0 | 0 | Agent ran and produced the required artifact. |
| `photonic-waveguide-routing` | 0 | 0 | Agent ran; the required routing artifact was absent. |
| `mp-checkpoint-consolidation` | 0 | 0 | Agent ran. |
| `ks-solver-cpp` | 0 | 1 | Codex failed with `workspace routing discovery failed`. |
| `react-lead-form` | 0 | 1 | Codex failed with `workspace routing discovery failed`. |
| `biped-contact-dynamics` | — | — | Trial interrupted before an agent exit marker. |
| `lake-temp-glm` | — | — | Trial interrupted before an agent exit marker. |
| `takens-embedding-lean`, `satb-audio-transcription`, `formal-crypto` | — | — | Not started. |

The single-agent record is under `/tmp/wteam-tb-luna-single10-20261028/jobs/` on the benchmark host. The three completed agent episodes with exit code 0 each earned reward 0; this is a partial observation, not a ten-task benchmark score.

## Scoring fault found after the attempts

The custom Harbor agent ignored `environment.exec()` return codes. For example, the single-arm `react-lead-form` trial has `agent/exit_code.txt` set to `exit=1` and a Codex `turn.failed` event with `workspace routing discovery failed`, yet Harbor recorded reward 0 with `exception_info: null`. The harness now raises on nonzero setup or agent exits and on a missing agent exit marker. A local fake-environment check confirmed these branches; a fresh Harbor run has not yet verified the change. The prior raw rewards remain invalid for comparison.

# Signals in pipelines other than wteam

Where to look when the run is not a wteam effort. Apply the method in `SKILL.md`.

| Symptom | Look at | Usual lever |
|---|---|---|
| Agent "used" a tool that never ran | Tool-call events and their results, not the agent's summary | Make the tool available to that stage, or say in the stage prompt when to call it |
| Stage repeats until timeout | Attempt count and timeout per node | Cap attempts (2 is often enough); lower the timeout; add a stop condition to the prompt |
| Stage finishes before its check | Workflow graph edge order | Put the verification node before the completion edge |
| One task eats the run | Task list sizes, tokens per task | Tracer-bullet first task; split tasks; state a size limit in the planner prompt |
| Slow stage adds no signal | Wall time per node vs what it produced | Forbid that step in that node (for example, tests in a planning node) |
| Cost spike from a tool | Tokens or spend per tool | Remove the tool from the stage, or swap it for a local or cached source |
| Fixes keep landing in engine code | Diff of the last refinements | Move the behaviour into roles, prompts, or config |

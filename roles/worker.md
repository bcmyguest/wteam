# worker

You own one ticket, delivered in your brief. Work red/green.

## Steps

1. Read the brief and run `wteam inbox` for earlier findings and notes.
2. Write a failing test for each acceptance criterion (red).
3. Write the code that passes it (green).
4. Run the project gate (fmt, lint and test as the repo defines them) and the ticket's `Verify:` command; all must pass.
5. Write the summary (what you built, what a reviewer should check) to `scratch/summary-<ticket>.md` with the file-editing tool. Then run `wteam submit <ticket> --summary-file scratch/summary-<ticket>.md`, adding `--extra <path>` for each changed file outside `Files:`.

You finish with exactly one of `wteam submit` or `wteam block <ticket> --reason-file <path>` (write the reason to `scratch/block-<ticket>.md` first, the same way). Ending a turn with neither stalls the run, so call `wteam block` the moment you cannot proceed: the ticket is ambiguous, a dependency is missing, or the work needs a file that clearly belongs to another ticket in progress.

## Rules

- Run exactly one command per shell call: no `&&`, `;` or `|`.
- Run commands from your working directory (the repo root): no `git -C`.
- Write scratch checks under `scratch/` in the repo root. Never write to `/tmp` or outside the repo: those writes are denied. Never submit `scratch/`: it is not in `Files:` and not an extra.
- Never write file contents through the shell: no redirection operator, no heredocs, no `printf` or `echo` into files.
- Never run a multi-line command.
- Keep `--note` values on one line.
- If `wteam` itself is denied, retry it once in its file form (`--summary-file`, `--reason-file`). If it is still denied, end your turn with one sentence naming the denied command.
- Never pass `scratch/` paths as `--extra`: scratch is never ticket work.
- `Files:` marks where the work belongs, not a fence. Touch a path outside it only when the work needs it, declare it with `--extra <path>`, and give the reason in the summary. Each extra is judged at review.
- After a failed review, the brief carries findings. Fix every finding, then submit again.

## Scratch checks

1. Run `mkdir -p scratch` as its own command.
2. Create `scratch/check_<name>.py` with the file-editing tool.
3. Run `python3 scratch/check_<name>.py` as one line.

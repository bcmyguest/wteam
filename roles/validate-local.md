# validate-local

Fresh session — local validation. You never saw the worker or reviewer chat.

## Job

1. Pick one ticket (the `--ticket` hint if given, else the newest `done` /
   `review` ticket that still looks weak). Read its contract and diff vs HEAD.
2. Run its `Verify:` command yourself. Check every acceptance bullet against the
   tree — do not trust prior review mail.
3. FAIL with concrete `- ` bullets (ticket id + fix) when Verify fails, a bullet
   is unmet, or the diff does not match What to build. PASS only when local
   evidence is green.

## Rules

- Do not edit source code. Write only under `.wteam/` (findings file).
- Run exactly one command per shell call: no `&&`, `;` or `|`.
- Finish with exactly one of:
  - `wteam phase-verdict pass --file <path>`
  - `wteam phase-verdict fail --file <path>`
- The file must contain a `FINDINGS:` section. FAIL needs at least one bullet.

# spec-readiness

Fresh session — go/no-go check before workers run. You never saw the planner's
chat.

## Job

1. Read `spec.md` and every ticket under `tickets/`.
2. Verify implementation readiness: every ticket has concrete `Files:`, a
   `Verify:` command a worker can run, testable acceptance criteria, and a
   tracer (01) that unlocks parallel work when the domain allows.
3. FAIL if a worker would have to invent paths, signatures, or behaviour not in
   the artifacts. PASS only when the plan is ready to transcribe into code.

## Rules

- Do not edit source code. Write only under `.wteam/` (findings file).
- Run exactly one command per shell call: no `&&`, `;` or `|`.
- Finish with exactly one of:
  - `wteam plan-verdict pass --file <path>`
  - `wteam plan-verdict fail --file <path>`
- The file must contain a `FINDINGS:` section. FAIL needs at least one bullet.

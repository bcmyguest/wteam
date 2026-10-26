# validate-out

Fresh session — outcome validation. You never saw the planning or implementation
chats.

## Job

1. Read the Outcome (and Problem Statement) in `spec.md`. Run the **full project
   gate** the repo uses (for Rust: `cargo fmt --check` then `cargo test`, or the
   Verify commands that stand in for a grade). When `spec.md` or Verify names a
   single gate script or command, run that instead of inventing a narrower check.
   Do not stop at a single ticket's filter.
2. Map each failure to ticket id(s) the orchestrator should `requeue`. Prefer the
   owning ticket's Files / Verify over inventing new work.
3. FAIL with concrete `- ` bullets (ticket id + what broke). PASS only when the
   Outcome looks met and the full gate is green.

## Rules

- Do not edit source code. Write only under `.wteam/` (findings file).
- Run exactly one command per shell call: no `&&`, `;` or `|`.
- Finish with exactly one of:
  - `wteam phase-verdict pass --file <path>`
  - `wteam phase-verdict fail --file <path>`
- The file must contain a `FINDINGS:` section. FAIL needs at least one bullet.

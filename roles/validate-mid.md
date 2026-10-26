# validate-mid

Fresh session — integration validation. You never saw the workers' chats.

## Job

1. Read `spec.md` and every ticket. Walk done tickets for cross-ticket issues:
   `Files:` overlap lies, weak `Blocked by`, Verify commands that never exercise
   the claimed behaviour together.
2. Run the integration-relevant Verify commands (and any project tests those
   tickets name). Prefer failing a mid-level assertion over assuming local PASS
   stamps are enough.
3. FAIL with concrete `- ` bullets naming ticket ids to requeue. PASS only when
   the tickets cohere and their shared Verify surface is green.

## Rules

- Do not edit source code. Write only under `.wteam/` (findings file).
- Run exactly one command per shell call: no `&&`, `;` or `|`.
- Finish with exactly one of:
  - `wteam phase-verdict pass --file <path>`
  - `wteam phase-verdict fail --file <path>`
- The file must contain a `FINDINGS:` section. FAIL needs at least one bullet.

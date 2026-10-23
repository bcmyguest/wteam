# incidentkit — build spec

## Outcome

The `incidentkit` package (Python 3.11+, standard library only) implements `SPEC.md` (v2)
exactly: every module of SPEC.md §0, every public name, signature, rule, example and
error. A hidden test suite derived from SPEC.md grades the result, so SPEC.md is the
single source of truth. Each ticket cites the SPEC.md sections it implements; workers read
those sections in full before coding.

Modules: `errors`, `timeutil`, `model`, `parsers`, `timeline`, `correlate`, `metrics`,
`query`, `report`, `__main__`, `routing`, `oncall`, `escalation`, `slo`, `lifecycle`.

## Constraints

- Standard library only. No third-party imports, no packaging changes needed.
- Exact public names from SPEC.md, imported from the modules SPEC.md names.
- All instants are int ms UTC. SLO arithmetic uses `fractions.Fraction` only.
- Tests are hidden: each worker writes its own checks under `scratch/` (files created with
  the file-editing tool, run with one-line `python3` commands) and checks every SPEC.md
  example in its sections.
- Workers run exactly one command per shell call (no `;`, `&&`, `|`, multi-line).
- Shared types are written once by the tracer (ticket 01) with the exact field lists and
  orders of SPEC.md; later tickets fill in behaviour and must not rename or reorder fields.
- Modules import each other at module level only along this acyclic direction:
  errors → timeutil → model → timeline → parsers/query/routing/oncall/escalation/slo →
  lifecycle → correlate → metrics/report → `__main__`. Where a lower module needs a type
  from a higher one only for annotations, do not import it.

## Non-goals

- No time-zone database (fixed offsets only, SPEC §12.2).
- No packaging/distribution (setup.py, pyproject) and no changes to SPEC.md.
- No public API beyond SPEC.md; private helpers are free.

## Dispatch

12 tickets: tracer 01, then 02–11 fan out by module, then integration 12 blocked by all.
Only 09 (correlate) has an extra edge (on 08 lifecycle, whose transition functions it
drives). Tickets touch disjoint files except 01 and 12, so serial dispatch suffices.

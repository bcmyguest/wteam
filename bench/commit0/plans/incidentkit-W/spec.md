# bench: implement incidentkit

## Outcome

The `incidentkit` package implements `SPEC.md` exactly: every module in §0, every
public name, signature, rule and error. A hidden test suite derived from SPEC.md grades
the result, so `SPEC.md` is the source of truth; on any doubt, re-read the relevant
section rather than guessing.

Modules: `errors`, `timeutil`, `model`, `parsers`, `timeline`, `correlate`, `metrics`,
`query`, `report`, `__main__` (CLI).

## Constraints

- Python 3.11+, standard library only. No third-party imports, no packaging changes needed.
- Public names live in the module SPEC.md names (tests import them from there).
- Instants are int milliseconds since epoch UTC everywhere.
- Workers write their own checks under `scratch/` (created with the file-editing tool) and
  run them with one-line `python3` commands. One command per shell call; no `;`, `&&`, `|`.
- Modules meet through the signatures in SPEC.md; ticket 01 creates every module with
  those exact signatures (stubs raise `NotImplementedError`) so all later tickets can run
  in parallel against an importable package.

## Shape of the work

- 01 tracer: `errors.py`, `model.py`, `timeline.py` complete (shared types), every other
  module stubbed with exact signatures and the shared dataclasses (`ParseResult`,
  `ParseError`, `Incident`) defined.
- 02–08 fan out by module: timeutil, parsers, correlate, metrics, query, report, CLI.
- 09 integration: exercise the whole public API end to end and fix seams.

## Non-goals

- No README/docs beyond SPEC.md; no packaging (`pyproject`) work.
- No committed test suite is required (hidden tests grade); scratch checks are fine.
- No behaviour beyond SPEC.md (no extra CLI commands, no extra formats).

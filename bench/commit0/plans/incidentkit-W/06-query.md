# 06: query — lexer, parser, evaluator, stages

**What to build:** Implement SPEC §8 in `incidentkit/query.py`: a lexer (whitespace, case-insensitive keywords, double-quoted strings with only `\"` and `\\` escapes, integers `-?[0-9]+`, identifiers `[A-Za-z_][A-Za-z0-9_.-]*`, operators `= != < <= > >= ~ ( ) |`), each token carrying its 0-based start offset; a recursive-descent parser for `query := [expr] ("|" stage)*` with `or` < `and` < `not` precedence; stages `head N`, `tail N`, `sort FIELD [desc]`, `dedupe DURATION` (a run of non-space characters parsed by `parse_duration`, its failure a `QueryError` at that token), `count`, `count by FIELD` (count stages must be last: `QueryError` at the next `|`). `compile(query)` returns an object with `.run(timeline)`; `run(query, timeline)` = `compile(query).run(timeline)`. All errors are `QueryError(msg, pos)` raised at compile time (including value conversion: bad severity, out-of-range int severity, bad ts string, integer for string field, `~` on severity/ts, unknown field); unexpected end → `pos = len(query)`.

Semantics: severity compared by level; ts numeric (string via `parse_ts`, int as ms); other fields as strings, `~` case-insensitive substring; missing fingerprint/attr → every comparison false (incl. `!=`). `sort` stable, missing first ascending, `desc` fully reversed order of the ascending sort keys while keeping incoming order among equals. `dedupe` same as `Timeline.dedupe`. `count by` → `[(value_str, n)]` sorted by count desc then value asc; severity → lowercase name, ts → `str(ms)`, missing → `"-"`. Empty/whitespace query returns all events (as a list).

**Blocked by:** 01

**Status:** ready-for-agent

**Files:** incidentkit/query.py

**Context:**
`SPEC.md:302  §8 Query language  — lexical rules, grammar, semantics, stages`
`incidentkit/query.py  compile(query) -> object with .run(timeline); run(query, timeline)  — stubs from 01`
`incidentkit/errors.py  class QueryError(ValueError) — QueryError(message, pos), attribute pos: int`
`incidentkit/timeline.py  Timeline(events=()), .events, .dedupe(window) -> Timeline  — from 01`
`incidentkit/model.py  Event fields ts/kind/service/severity/message/source/fingerprint/attrs; Event.attr(key, default=None)`
`incidentkit/model.py  Severity.parse(text); Severity(n)`
`incidentkit/timeutil.py  parse_ts(text, ...) -> int; parse_duration(text) -> int  — ticket 02`

- [ ] `run('service = web and severity >= high', t)` filters correctly; `severity >= 3` and `severity = "sev2"` work.
- [ ] `not` binds tighter than `and`, `and` tighter than `or`; parentheses group.
- [ ] `fingerprint != "x"` is false for events with fingerprint None; `not fingerprint = "x"` is true for them; `attr.host = web1` works.
- [ ] `message ~ BOOM` matches case-insensitively; `severity ~ x` and `ts ~ x` raise `QueryError`.
- [ ] `| head 2`, `| tail 2`, `| sort ts desc`, `| sort fingerprint` (missing first), `| dedupe 5m`, `| count` (int), `| count by service` (sorted list of tuples) behave per spec.
- [ ] `QueryError.pos` is the offset of the offending token: unknown field, bad operator, unterminated string, bad escape, `count | head 1` (pos of `|`), trailing `and` (pos = len(query)).
- [ ] Empty query returns all events as a list; `compile` raises before any timeline is given.

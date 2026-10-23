# 03: Query language

**What to build:** Implement `incidentkit/query.py` per SPEC §8 (lines 454–527): a lexer
(whitespace, case-insensitive keywords, double-quoted strings with only `\"`/`\\`
escapes, integers `-?[0-9]+`, identifiers `[A-Za-z_][A-Za-z0-9_.-]*`, operators), a
recursive-descent parser for the grammar (`not` > `and` > `or`, parentheses, fields
incl. `attr.<name>`), and stages `head N`, `tail N`, `sort FIELD [desc]`,
`dedupe DURATION` (via `Timeline.dedupe`), `count`, `count by FIELD`. `compile(query)`
returns an object with `.run(timeline)`; `run(query, timeline)` is `compile(query).run`.
All query errors are raised at compile time as `QueryError` with `pos` = offset of the
offending token (or `len(query)` for unexpected end). Comparison semantics: severity via
`Severity.parse`/`Severity(n)`, ts via `parse_ts` or integer ms, others as strings, `~`
case-insensitive substring and disallowed on severity/ts, missing fingerprint/attr makes
every comparison false. Sorting: stable, missing first ascending, full reverse with
`desc` while keeping incoming order among equal keys. `count by` sorted by count desc
then value asc; severity names lowercase, ts as `str(ms)`, missing as `"-"`.

**Blocked by:** 01

**Status:** ready-for-agent

**Files:** incidentkit/query.py

**Context:**
`SPEC.md:456  run(query, timeline) / compile(query).run(timeline)`
`SPEC.md:462  lexical rules`
`SPEC.md:474  grammar`
`SPEC.md:494  comparison semantics`
`SPEC.md:508  stages`
`incidentkit/errors.py  QueryError(message, pos)  — .pos offset`
`incidentkit/timeline.py  Timeline.dedupe(window) -> Timeline  — dedupe stage`
`incidentkit/timeline.py  Timeline.events -> list[Event]`
`incidentkit/timeutil.py  parse_ts(text) / parse_duration(text)  — ts values, dedupe durations`
`incidentkit/model.py  Event.attr(key, default=None); Severity.parse`

- [ ] empty or whitespace query returns all events in order
- [ ] `service = api and not severity < high or kind = deploy` respects precedence
- [ ] `fingerprint != x` is false for events with fingerprint None, and `not fingerprint = x` is true
- [ ] `severity ~ hi` raises QueryError at the offset of `~`; `count | head 1` raises at the offset of the second `|`; `service =` raises with `pos == len(query)`
- [ ] `sort fingerprint desc` puts missing values last and keeps incoming order among equal keys
- [ ] `count by severity` returns `[(name, n), ...]` sorted by count desc then value asc
- [ ] string `"a\"b\\"` parses; `"a\n"` (backslash-n) and unterminated strings raise QueryError

# 02: Parsers — alerts JSONL, syslog, changes CSV, load

**What to build:** Implement `incidentkit/parsers.py` per SPEC §4 (lines 186–266)
including the (v2) rules: `parse_alerts` (JSON Lines, source `alerts`, `type` trigger/
ack/resolve/**note**, `labels` → sorted attrs with bool/int/float conversion, the ack
`by` attr that replaces a `by` label and errors on a bad value, integer `at` via the
epoch digit rule, booleans rejected), `parse_syslog` (15-char timestamp, host,
service[pid], `<LEVEL>: ` prefix, trailing whitespace stripped, attrs host/pid, year
rollover > 180 days, repeatable), `parse_changes` (csv module, case-insensitive header,
missing required column → `ValueError`, empty file → empty result, line = CSV line where
the record starts, risk mapping, wrong field count is an error, attrs author/ticket), and
`load(path, *, year=None, tz="+00:00")` dispatching on `.jsonl`/`.log`/`.csv`.
Bad records are collected as `ParseError(line, reason)`; blank lines are skipped. Attrs
are always a tuple of (key, value) pairs sorted by key with unique keys.

**Blocked by:** 01

**Status:** ready-for-agent

**Files:** incidentkit/parsers.py

**Context:**
`SPEC.md:186  §4 ParseResult / ParseError and error collection rules`
`SPEC.md:194  §4.1 parse_alerts(text) -> ParseResult  — key table, v2 note type, ack by`
`SPEC.md:222  §4.2 parse_syslog(text, *, year, tz="+00:00") -> ParseResult  — line grammar, rollover`
`SPEC.md:247  §4.3 parse_changes(text) -> ParseResult  — CSV columns, risk`
`SPEC.md:262  §4.4 load(path, *, year=None, tz="+00:00") -> ParseResult`
`incidentkit/timeutil.py  parse_ts(text, *, default_year=None, default_tz="+00:00") -> int  — all timestamps`
`incidentkit/model.py  Severity.parse(text) -> Severity  — sev / LEVEL / risk mapping`
`incidentkit/model.py  Event(ts, kind, service, severity, message, source, fingerprint=None, attrs=())`
`incidentkit/errors.py  TimeParseError  — bad at/time becomes a ParseError, never raised`

- [ ] the §4.1 v2 example line yields `Event(0, "note", "api", Severity.MEDIUM, "/mitigate INC-0001", "alerts")`
- [ ] an ack with `"by": "bob"` and a label `by` yields attr `by=bob`; an ack with `"by": ""` or `"by": 1` is a ParseError; `by` on a trigger is ignored
- [ ] labels `true`, `3`, `1.5` become `"true"`, `"3"`, `"1.5"`; a list label is an error; `"at": true` is an error
- [ ] syslog `Mar  1 12:00:00 h1 api[42]: ERROR: boom  ` gives severity HIGH, message `boom`, attrs host/pid; a Dec→Jan sequence rolls the year
- [ ] changes CSV with a quoted multi-line field reports later records at the correct line numbers; missing `change` column raises `ValueError`
- [ ] `load` dispatches by extension, raises `ValueError` for `.log` without year and for unknown extensions

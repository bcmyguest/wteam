# 03: parsers — alerts JSONL, syslog, changes CSV, load

**What to build:** Implement SPEC §4 in `incidentkit/parsers.py`: `parse_alerts`, `parse_syslog` (with year rollover: if a line's ts is more than 180 days earlier than the previous successfully parsed line, bump the year for it and all later lines; can happen repeatedly), `parse_changes` (csv module; header case-insensitive and stripped; missing required column → `ValueError`; no header → empty result; line number = CSV line where the record starts, so track `reader.line_num` before each record; wrong field count is an error) and `load` (dispatch on `.jsonl`/`.log`/`.csv`, UTF-8, `.log` without year → `ValueError`, other extension → `ValueError`). Bad records are collected as `ParseError(line, reason)`, never raised; blank lines ignored. Attrs are tuples sorted by key. Keep the `ParseResult`/`ParseError` dataclasses from 01 unchanged.

Details to honour: JSON `at` may be a string (via `parse_ts`, no default year) or a JSON int (not bool) read with the epoch digit rule (use `parse_ts(str(n))`); `labels` values: bool → `"true"`/`"false"`, int → `str`, float → `repr`, other → error; `dedup_key` `""` → `None`; `svc` must be a non-empty string; `sev` absent → MEDIUM. Syslog: timestamp = first 15 chars; `service` excludes `[`, `]`, `:`; optional `[digits]`; then literal `: `; `<LEVEL>: ` prefix only when `Severity.parse` accepts it; trailing whitespace stripped from message; attrs `host` and optional `pid`. Changes: `risk` low/medium/high → LOW/MEDIUM/HIGH, empty/absent → INFO, other → error; attrs `author`, `ticket` when non-empty after stripping.

**Blocked by:** 01

**Status:** ready-for-agent

**Files:** incidentkit/parsers.py

**Context:**
`SPEC.md:132  §4 Parsers  — normative rules`
`incidentkit/parsers.py  @dataclass ParseError(line: int, reason: str); @dataclass ParseResult(events: list[Event], errors: list[ParseError])  — from 01`
`incidentkit/parsers.py  parse_alerts(text) / parse_syslog(text, *, year, tz="+00:00") / parse_changes(text) / load(path, *, year=None, tz="+00:00")  — stubs from 01`
`incidentkit/model.py  Event(ts, kind, service, severity, message, source, fingerprint=None, attrs=())  — frozen dataclass`
`incidentkit/model.py  Severity.parse(text) -> Severity  — raises ValueError`
`incidentkit/timeutil.py  parse_ts(text, *, default_year=None, default_tz="+00:00") -> int  — raises TimeParseError (ticket 02)`

- [ ] `parse_alerts` maps trigger/ack/resolve to alert/ack/resolve with source `"alerts"`, defaults sev MEDIUM, summary `""`, fingerprint None; labels converted and sorted into attrs.
- [ ] Invalid JSON, non-object, missing/ill-typed required key, unknown type, bad `at`, bad `sev`, bool `at`, list label value each produce one `ParseError` with the right 1-based line, and other lines still parse.
- [ ] `parse_syslog` parses `Mar  3 01:02:03 web1 nginx[123]: ERROR: boom ` → severity HIGH, message `"boom"`, attrs `(("host","web1"),("pid","123"))`; without level prefix severity INFO.
- [ ] Syslog year rollover: `Dec 31 …` then `Jan  1 …` with `year=2023` gives the second line in 2024.
- [ ] `parse_changes` handles quoted fields, case-insensitive header, risk mapping, attrs; missing required column raises `ValueError`; empty text returns empty result; error line numbers count multi-line quoted records correctly.
- [ ] `load` dispatches on extension, raises `ValueError` for `.log` without year and unknown extensions.

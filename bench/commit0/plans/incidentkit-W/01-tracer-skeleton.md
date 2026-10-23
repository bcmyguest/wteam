# 01: Tracer — importable skeleton, errors, model, timeline

**What to build:** The skeleton every other ticket builds on. Create every module of SPEC.md §0 so `import incidentkit.<m>` works for all of them, with every public name defined at its exact signature.

Fully implement (these are shared types; implement them completely here):
- `incidentkit/errors.py` (SPEC §1): `TimeParseError`, `DurationParseError`, `QueryError(ValueError)` with attribute `pos: int` (constructor `QueryError(message, pos)`).
- `incidentkit/model.py` (SPEC §3): `Severity` IntEnum with `Severity.parse` and all aliases; module-level `KIND_ORDER`; frozen dataclass `Event` with fields in order, `attr()`, `sort_key()`.
- `incidentkit/timeline.py` (SPEC §5): the whole `Timeline` class — `__init__`, `add`, `__len__`, `__iter__`, `__getitem__` (int → Event, slice → list), `events`, `services`, `between`, `filter`, `dedupe`, `gaps`, `bucket` (it is one coupled class; do not leave parts for later).
- Shared dataclasses: `ParseError(line, reason)` and `ParseResult(events, errors)` in `parsers.py`; `Incident` dataclass with all fields and properties (`services`, `fingerprints`, `severity`, `title`, `is_open`) in `correlate.py`.

Stub (body `raise NotImplementedError`, exact signature, so later tickets fill them in):
- `timeutil.py`: `parse_ts(text, *, default_year=None, default_tz="+00:00")`, `parse_duration(text)`, `format_ts(ms)`, `format_duration(ms)`.
- `parsers.py`: `parse_alerts(text)`, `parse_syslog(text, *, year, tz="+00:00")`, `parse_changes(text)`, `load(path, *, year=None, tz="+00:00")`.
- `correlate.py`: `correlate(timeline, *, window=300_000)`, `suspect_changes(incident, timeline, *, lookback=3_600_000)`.
- `metrics.py`: `percentile(values, p)`, `mtta`, `mttr`, `summary`, `by_service`, `availability(incidents, service, start, end)`.
- `query.py`: `compile(query)`, `run(query, timeline)`.
- `report.py`: `render(incidents, timeline, *, title="Incident report", lookback=3_600_000)`.
- `__main__.py`: `main(argv=None) -> int` stub plus `if __name__ == "__main__": sys.exit(main())`.

Leave `incidentkit/__init__.py` empty (or a docstring). Standard library only.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

**Files:** incidentkit/__init__.py, incidentkit/errors.py, incidentkit/model.py, incidentkit/timeline.py, incidentkit/timeutil.py, incidentkit/parsers.py, incidentkit/correlate.py, incidentkit/metrics.py, incidentkit/query.py, incidentkit/report.py, incidentkit/__main__.py

**Context:**
`SPEC.md:12   §0 Package layout  — module list`
`SPEC.md:28   §1 Errors  — TimeParseError, DurationParseError, QueryError.pos`
`SPEC.md:86   §3 Model  — Severity, Severity.parse aliases, Event, KIND_ORDER`
`SPEC.md:134  ParseResult(events: list[Event], errors: list[ParseError]); ParseError(line: int, reason: str)`
`SPEC.md:201  §5 Timeline(events=())  — sorted stable container, dedupe/gaps/bucket`
`SPEC.md:228  Incident(id, opened_at, acked_at, resolved_at, alerts)  — mutable dataclass + properties`
`SPEC.md:42   parse_ts / parse_duration / format_ts / format_duration signatures`
`SPEC.md:275  metrics signatures`
`SPEC.md:304  query.run(query, timeline), query.compile(query).run(timeline)`
`SPEC.md:375  report.render signature`
`incidentkit/__init__.py:1  (empty)  — package marker`

- [ ] Every module in SPEC §0 imports without error and exposes its public names.
- [ ] `QueryError("x", 3).pos == 3` and all three errors are `ValueError` subclasses.
- [ ] `Severity.parse(" SEV1 ") is Severity.CRITICAL`, `Severity.parse("warn") is Severity.MEDIUM`, unknown text raises `ValueError`.
- [ ] `Event` is frozen, hashable, equal by fields; `fingerprint` defaults `None`, `attrs` `()`; `attr()` and `sort_key()` per spec.
- [ ] `Timeline` sorts stably by `sort_key`; `add` puts new equal-key events after existing ones; slicing returns a list.
- [ ] `Timeline.dedupe(50)` on identities at ts 0, 40, 80 keeps 0 and 80; fingerprint identity used when not `None`.
- [ ] `gaps`, `between`, `filter`, `services` per spec; `bucket` floors start (works for negative ts), includes empty buckets, `width <= 0` raises `ValueError`, empty timeline → `[]`.
- [ ] `Incident` properties `services`, `fingerprints`, `severity`, `title`, `is_open` behave per SPEC §6.

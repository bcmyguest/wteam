# 09: Integration — whole public API end to end, fix seams

**What to build:** Exercise the full public API end to end and fix whatever breaks between modules. Write scratch fixtures (an alerts `.jsonl`, a syslog `.log` crossing a year boundary, a changes `.csv`) under `scratch/`, then: load all three, merge into a `Timeline`, dedupe, `correlate`, compute `summary`/`by_service`/`availability`, run a range of queries, `render` a report, and drive the CLI (`python3 -m incidentkit report|stats|query ...`) checking output and exit codes. Re-check every example literally stated in SPEC.md (§2 timestamp/duration examples, §5 dedupe example, §9 template, §10 exit codes). Re-read SPEC.md top to bottom against the code and fix any deviation in names, signatures, defaults, error types or ordering. Confirm only the standard library is imported.

**Blocked by:** 01, 02, 03, 04, 05, 06, 07, 08

**Status:** ready-for-agent

**Files:** incidentkit/__init__.py, incidentkit/errors.py, incidentkit/timeutil.py, incidentkit/model.py, incidentkit/parsers.py, incidentkit/timeline.py, incidentkit/correlate.py, incidentkit/metrics.py, incidentkit/query.py, incidentkit/report.py, incidentkit/__main__.py

**Context:**
`SPEC.md:1    whole spec  — normative source of truth`
`incidentkit/parsers.py  load(path, *, year=None, tz="+00:00") -> ParseResult`
`incidentkit/timeline.py  Timeline(events=()), add, dedupe, between, bucket, gaps`
`incidentkit/correlate.py  correlate(timeline, *, window=300_000); suspect_changes(...); Incident`
`incidentkit/metrics.py  percentile, mtta, mttr, summary, by_service, availability`
`incidentkit/query.py  compile(query).run(timeline); run(query, timeline)`
`incidentkit/report.py  render(incidents, timeline, *, title, lookback)`
`incidentkit/__main__.py  main(argv=None) -> int`
`incidentkit/timeutil.py  parse_ts, parse_duration, format_ts, format_duration`

- [ ] Every literal example in SPEC.md evaluates to the stated result.
- [ ] Loading the three fixture formats, correlating and rendering produces a report matching the §9 template byte for byte (checked against a hand-written expected string).
- [ ] CLI `report`, `stats`, `query` produce the documented stdout and exit codes 0/1/2 on the fixtures.
- [ ] No module imports anything outside the standard library and `incidentkit`.

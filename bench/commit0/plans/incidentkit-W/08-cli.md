# 08: CLI — python -m incidentkit report/stats/query

**What to build:** Implement SPEC §10 in `incidentkit/__main__.py` as `main(argv=None) -> int` with `sys.exit(main())` under `__main__`. Subcommands `report`, `stats`, `query` with options `--year Y` (int, default None), `--tz TZ` (default `+00:00`), `--window DUR` (default `5m`, report/stats), `--dedupe DUR` (report/stats), `--title T` (report), `--strict`; positional `FILE...` (query: `QUERY FILE...`). Load each file with `parsers.load`, merge into one `Timeline`, print each parse error to stderr as `{path}:{line}: {reason}`. `--dedupe` applies `Timeline.dedupe` before correlation. `report` prints `render(...)` with no extra newline (`sys.stdout.write`); `stats` prints `json.dumps(summary(incidents), indent=2)` + newline; `query` prints events as `{format_ts(ts)} {kind} {service} {severity lowercase} {message}`, ints as the number, count-by as `{value}\t{count}` lines. Exit 0 success; 1 if `--strict` and any parse error (output still printed); 2 on usage error, unreadable file, `ValueError` from `load`, invalid duration or `QueryError`, with a one-line stderr message and nothing on stdout. Make argparse usage errors return 2 without printing to stdout (subclass ArgumentParser or catch `SystemExit`); compile the query and parse durations before printing anything.

**Blocked by:** 01

**Status:** ready-for-agent

**Files:** incidentkit/__main__.py

**Context:**
`SPEC.md:422  §10 Command line  — normative`
`incidentkit/__main__.py  main(argv=None) -> int  — stub from 01`
`incidentkit/parsers.py  load(path, *, year=None, tz="+00:00") -> ParseResult(events, errors[ParseError(line, reason)])  — ticket 03`
`incidentkit/timeline.py  Timeline(events=()), .add(events), .dedupe(window)`
`incidentkit/correlate.py  correlate(timeline, *, window=300_000) -> list[Incident]  — ticket 04`
`incidentkit/metrics.py  summary(incidents) -> dict  — ticket 05`
`incidentkit/query.py  compile(query).run(timeline) -> list[Event] | int | list[tuple[str,int]]  — ticket 06`
`incidentkit/report.py  render(incidents, timeline, *, title=...) -> str  — ticket 07`
`incidentkit/timeutil.py  parse_duration(text); format_ts(ms)  — ticket 02`
`incidentkit/errors.py  QueryError, DurationParseError`

- [ ] `python -m incidentkit stats FILE` prints indented JSON of `summary` and exits 0.
- [ ] `report --title T FILE` prints the rendered report exactly (no extra trailing newline).
- [ ] `query 'kind = alert | count by service' FILE` prints `value\tcount` lines; event output lines match the spec format; `| count` prints an int.
- [ ] A file with a bad record prints `path:line: reason` to stderr; exit 0 normally, 1 with `--strict`, stdout still printed.
- [ ] Exit 2 with empty stdout and one stderr line for: missing file, unknown extension, `.log` without `--year`, `--window 5x`, bad query, unknown subcommand/missing args.

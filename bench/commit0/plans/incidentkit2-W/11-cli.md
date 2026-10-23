# 11: Command line (`python -m incidentkit`)

**What to build:** Implement `incidentkit/__main__.py` per SPEC §10 (lines 592–732) with
`argparse` subcommands `report`, `stats`, `query`, `route`, `oncall`, `escalate`, `slo`
and a `main(argv=None) -> int` entry point (`sys.exit(main())`). Loading via `load`,
parse errors to stderr as `{path}:{line}: {reason}`, `--strict` exit 1, exit 2 for usage
errors / unreadable files / `ValueError` from load / invalid durations / `QueryError` /
`TimeParseError` / `RoutingError` (message `{rules_path}:{line}: {reason}`) /
`ScheduleError` / `PolicyError` / `SloError`, with nothing on stdout. The §10.1 pipeline
order for report/stats/escalate (rules and config read before FILEs; dedupe; `--slo`
requires `--config`, feed with start/end from the timeline; `check_references` errors
`{rules_path}:{line}: unknown policy {name}`; correlate with all keywords; negative
`--reopen` exit 2; `--timeline` passes the audit log to render). Output formats of
`query`, `route` (§10.2), `oncall` (§10.3, `--at` xor `--from/--to`, schedule order by
name), `escalate` (§10.4, tab-separated) and `slo` (§10.5, `--from` must be < `--at`,
burn alerts ordered as a Timeline). argparse's own usage errors already exit 2. Output
must be buffered so that an error exit prints nothing on stdout.

**Blocked by:** 01

**Status:** ready-for-agent

**Files:** incidentkit/__main__.py

**Context:**
`SPEC.md:592  §10 v1 commands, output formats and exit codes`
`SPEC.md:615  §10 v2 general rules (any order, TS parsing, file errors)`
`SPEC.md:622  §10.1 report/stats pipeline`
`SPEC.md:655  §10.2 route`
`SPEC.md:677  §10.3 oncall`
`SPEC.md:693  §10.4 escalate`
`SPEC.md:709  §10.5 slo`
`SPEC.md:1679 §15.7 escalate CLI example`
`incidentkit/parsers.py  load(path, *, year=None, tz="+00:00") -> ParseResult`
`incidentkit/timeline.py  Timeline(events); add; dedupe(window)`
`incidentkit/correlate.py  correlate(timeline, *, window, rules, schedules, policies, reopen_window, audit)`
`incidentkit/metrics.py  summary(incidents)`
`incidentkit/report.py  render(incidents, timeline, *, title, audit)`
`incidentkit/query.py  run(query, timeline)`
`incidentkit/routing.py  load_rules(path); routes_for(ruleset, event)`
`incidentkit/oncall.py  load_schedules(text); who_is_on_call; final_schedule`
`incidentkit/escalation.py  load_policies(text, schedules); check_references(ruleset, policies)`
`incidentkit/slo.py  load_slos(text); feed; status; burn_alerts; format_percent`
`incidentkit/lifecycle.py  AuditLog()`
`incidentkit/timeutil.py  parse_ts; parse_duration; format_ts`

- [ ] `stats` prints `json.dumps(summary, indent=2)` plus newline; `report` prints render output with no extra newline
- [ ] `query` prints events, an int, or `value\tcount` lines; a bad query exits 2 with empty stdout
- [ ] `--strict` with a parse error exits 1 and still prints output; an unknown extension or unreadable file exits 2
- [ ] `route` prints the §10.2 example line for the §11.6 rules
- [ ] `oncall --at` and `--from/--to` print the §10.3 formats; `--at` with `--from`, or `--to <= --from`, or an unknown `--schedule` exits 2
- [ ] `escalate` with the §15.7 inputs and missing policies exits 2 with `unknown policy` lines on stderr; with them added prints the three §15.7 lines
- [ ] `slo` prints the status line format and, with `--from`, the burn alert lines; `--from >= --at` exits 2; `--slo` without `--config` exits 2

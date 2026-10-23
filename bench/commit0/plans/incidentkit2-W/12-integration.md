# 12: Integration — whole public API end to end

**What to build:** Exercise the whole public API end to end against SPEC.md and fix the
seams between tickets. Write scratch checks (under `scratch/`) that reproduce every
worked example in SPEC.md across module boundaries: §11.6 routing table, §11.7 teams
example, §12.7 schedule, §13.7 escalation, §14.7 SLO (including feeding into correlate:
INC-0001 opened 10:01, resolved 10:06), §15.3 and §15.5 examples, and the full §15.7
worked example (JSONL file → `parse_alerts` → `Timeline` → `correlate` with rules,
config, reopen 10m and an `AuditLog` → `summary` dict, `render(..., audit=...)` block,
`audit.render()`), plus the CLI commands `report`, `stats`, `query`, `route`, `oncall`,
`escalate`, `slo` run with `python3 -m incidentkit` on files written to `scratch/`.
Verify v1 compatibility: without v2 options/notes, outputs equal v1 except the two new
summary keys. Check that every public name in SPEC.md is importable from its module,
there are no import cycles, and only the standard library is used. Fix any mismatch in
whichever source file it lives in.

**Blocked by:** 01, 02, 03, 04, 05, 06, 07, 08, 09, 10, 11

**Status:** ready-for-agent

**Files:** incidentkit/__init__.py, incidentkit/errors.py, incidentkit/timeutil.py, incidentkit/model.py, incidentkit/timeline.py, incidentkit/parsers.py, incidentkit/correlate.py, incidentkit/metrics.py, incidentkit/query.py, incidentkit/report.py, incidentkit/__main__.py, incidentkit/routing.py, incidentkit/oncall.py, incidentkit/escalation.py, incidentkit/slo.py, incidentkit/lifecycle.py

**Context:**
`SPEC.md:19   §0 package layout and config-file rules`
`SPEC.md:863  §11.6 routing example`
`SPEC.md:928  §11.7 teams example`
`SPEC.md:1041 §12.7 schedule example`
`SPEC.md:1199 §13.7 escalation example`
`SPEC.md:1353 §14.7 SLO example`
`SPEC.md:1577 §15.5 reopen example`
`SPEC.md:1606 §15.7 full worked example (events, audit, summary, report block, escalate CLI)`
`incidentkit/parsers.py  parse_alerts(text); load(path, ...)`
`incidentkit/correlate.py  correlate(timeline, *, window, rules, schedules, policies, reopen_window, audit)`
`incidentkit/metrics.py  summary(incidents)`
`incidentkit/report.py  render(incidents, timeline, *, title, lookback, audit)`
`incidentkit/slo.py  feed(timeline, slos, start, end)`
`incidentkit/__main__.py  main(argv=None) -> int`

- [ ] the §15.7 pipeline reproduces the audit text, summary dict and incident block exactly
- [ ] the §14.7 fed timeline correlates to one incident opened at 2024-03-01T10:01:00Z and resolved at 10:06:00Z
- [ ] `python3 -m incidentkit escalate` on the §15.7 inputs exits 2 without `pay-low`/`dba-page` policies and prints the three lines with them
- [ ] every SPEC.md example in §2, §11, §12, §13, §14, §15 holds when called through the public modules
- [ ] every public name in SPEC.md imports from its module, with no import cycle and no non-stdlib import

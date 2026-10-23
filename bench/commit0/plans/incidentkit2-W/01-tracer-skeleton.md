# 01: Tracer — foundations, shared types and importable skeleton

**What to build:** Make every module of SPEC.md §0 importable with every public name
present, and fully implement the foundation that all other tickets build on:

- `incidentkit/errors.py` complete (SPEC §1, lines 51–75): `TimeParseError`,
  `DurationParseError`, `QueryError(msg, pos)` with `.pos`, `RoutingError(line, reason)`
  with `str == f"line {line}: {reason}"`, `ScheduleError`, `PolicyError`, `SloError`,
  `CommandError`, `IllegalTransition(incident_id, state, action)`, `ReopenWindowError`
  (subclass of `IllegalTransition`). All subclass `ValueError`.
- `incidentkit/timeutil.py` complete (SPEC §2): `parse_ts`, `parse_duration`,
  `format_ts`, `format_duration`, all examples. Also expose a private helper to parse a
  zone string (`Z`/`z`/`±HH:MM`) to offset ms, reused by oncall.
- `incidentkit/model.py` complete (SPEC §3): `Severity` (IntEnum + `parse` with aliases),
  `KIND_ORDER`, frozen `Event` with `attr()` and `sort_key()`.
- `incidentkit/timeline.py` complete (SPEC §5): `Timeline` with `add`, len/iter/index/
  slice, `events`, `services`, `between`, `filter`, `dedupe`, `gaps`, `bucket`.
- `incidentkit/correlate.py`: the full `Incident` dataclass (SPEC §6 + §6 v2 field table,
  lines 298–387: v1 fields then the 12 v2 fields in order with defaults,
  `notifications` via `field(default_factory=list)`), with the v2 properties `services`,
  `fingerprints`, `severity` (override first), `title` (alert max only), `is_open`
  (`resolved_at is None and merged_into is None`). `correlate` and `suspect_changes` are
  stubs with the exact v2 signatures raising `NotImplementedError`.
- Shared data types with exact fields/orders, defined in their home modules:
  `parsers.ParseResult(events, errors)`, `parsers.ParseError(line, reason)`;
  `routing.Route`, `routing.Rule` (fields `name, line, team, policy, cont` plus a
  `matchers: tuple = ()` storage field, `matches` stub), `routing.RuleSet(rules, default,
  default_line)`; `oncall.Schedule(name, tz_offset_ms, layers, overrides)`;
  `escalation.Level`, `escalation.Policy`, `escalation.Notification`; `slo.BurnRule`,
  `slo.Slo`, `slo.SloStatus`; `lifecycle.Command`, `lifecycle.AuditEntry` (with `line()`
  implemented), lifecycle state constants and `ACTIVE_STATES`.
- Every other public function/class named in SPEC.md exists in its module as a stub with
  the exact signature, raising `NotImplementedError` (parsers, query incl. `compile`,
  metrics, report.render, routing, oncall, escalation, slo, lifecycle incl. `AuditLog`).
- `incidentkit/__main__.py`: `main(argv=None) -> int` stub that prints a usage line to
  stderr and returns 2; `if __name__ == "__main__": sys.exit(main())`.
- `incidentkit/__init__.py` may stay empty.

Respect the import direction in spec.md (no cycles: e.g. `routing`, `oncall` must not
import `correlate`).

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

**Files:** incidentkit/__init__.py, incidentkit/errors.py, incidentkit/timeutil.py, incidentkit/model.py, incidentkit/timeline.py, incidentkit/parsers.py, incidentkit/correlate.py, incidentkit/metrics.py, incidentkit/query.py, incidentkit/report.py, incidentkit/__main__.py, incidentkit/routing.py, incidentkit/oncall.py, incidentkit/escalation.py, incidentkit/slo.py, incidentkit/lifecycle.py

**Context:**
`SPEC.md:19   §0 package layout  — module list`
`SPEC.md:51   §1 errors  — exception classes and constructors`
`SPEC.md:81   parse_ts(text, *, default_year=None, default_tz="+00:00") -> int`
`SPEC.md:104  parse_duration(text) -> int`
`SPEC.md:114  format_ts(ms) -> str`
`SPEC.md:123  format_duration(ms) -> str`
`SPEC.md:135  class Severity(IntEnum) / Severity.parse(text)`
`SPEC.md:160  @dataclass(frozen=True) class Event(ts, kind, service, severity, message, source, fingerprint=None, attrs=())`
`SPEC.md:186  ParseResult(events, errors), ParseError(line, reason)`
`SPEC.md:268  class Timeline(events=())  — §5`
`SPEC.md:298  @dataclass class Incident(id, opened_at, acked_at, resolved_at, alerts, ...v2 fields)`
`SPEC.md:358  §6 v2 new Incident fields table (order + defaults)`
`SPEC.md:377  §6 v2 changed properties`
`SPEC.md:391  correlate(timeline, *, window=300_000, rules=None, schedules=None, policies=None, reopen_window=None, audit=None)`
`SPEC.md:342  suspect_changes(incident, timeline, *, lookback=3_600_000)`
`SPEC.md:410  §7 metrics names`
`SPEC.md:456  query.run(query, timeline), query.compile(query)`
`SPEC.md:531  report.render(incidents, timeline, *, title=..., lookback=..., audit=None)`
`SPEC.md:837  §11.5 Route, Rule, RuleSet, parse_rules, load_rules, routes_for, route; §11.1 glob_match`
`SPEC.md:977  Schedule(name, tz_offset_ms, layers, overrides); load_schedules, who_is_on_call, final_schedule`
`SPEC.md:1119 Level(delay_minutes, schedule, users), Policy(name, levels, repeat), Notification(ts, level, user, attempt)`
`SPEC.md:1128 simulate, on_call_users, responder, escalate, check_references, load_policies`
`SPEC.md:1260 BurnRule(long_ms, short_ms, factor, severity), Slo(name, service, objective, window_days, bad_severity, burn_alerts)`
`SPEC.md:1288 SloStatus(name, total, bad, good, allowed_bad, remaining); load_slos, budget_fraction, bad_minutes, burn_rate, status, format_fixed, format_percent, burn_alerts, feed`
`SPEC.md:1385 lifecycle constants TRIGGERED..MERGED, ACTIVE_STATES`
`SPEC.md:1399 apply_open/ack/mitigate/resolve/reopen/severity/merge signatures`
`SPEC.md:1445 Command(name, incident, arg); parse_command(text)`
`SPEC.md:1473 AuditEntry(ts, incident, action, from_state, to_state, detail="").line(); AuditLog`

- [ ] `import incidentkit.<m>` succeeds for all 15 modules and every SPEC.md public name is an attribute of its module
- [ ] every `parse_ts`, `parse_duration`, `format_ts`, `format_duration` example in SPEC §2 holds; invalid inputs raise the named errors
- [ ] `Severity.parse` accepts every name and alias case-insensitively and raises `ValueError` otherwise
- [ ] `Event` is frozen, hashable, `sort_key()` uses `KIND_ORDER` with `note` last
- [ ] `Timeline` behaviours of SPEC §5 hold, including the dedupe example (window 50, ts 0/40/80) and `bucket` with negative ts and `width <= 0` → `ValueError`
- [ ] `Incident("INC-0001", 0, None, None, [e])` positional construction works; override example of §6 v2 gives severity HIGH and title "db down"
- [ ] `RoutingError(3, "x")` has `str == "line 3: x"`; `ReopenWindowError` is an `IllegalTransition` with attributes `incident_id`, `state`, `action`
- [ ] `AuditEntry.line()` renders the §15.4 format with and without detail
- [ ] `python3 -m incidentkit` exits 2 without a traceback

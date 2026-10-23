# 10: Metrics and Markdown report

**What to build:** Implement `incidentkit/metrics.py` per SPEC §7 incl. v2 (lines
410–452, 1190–1197): `percentile` (nearest rank, exact arithmetic with `Fraction` for
floats, `ValueError` outside [0, 100], `None` when empty), `mtta`, `mttr` (floor mean),
`summary` (9 keys in order, `by_severity` via the `severity` property, `escalated` =
incidents with a notification of `level >= 2`, `out_of_band_acks`), `by_service`
(sorted keys), `availability` (severity property `>= HIGH`, merged intervals, clipped,
`round(x, 6)`, `ValueError` for `end <= start`); every function skips incidents with
`merged_into` set. Implement `incidentkit/report.py` `render(incidents, timeline, *,
title="Incident report", lookback=3_600_000, audit=None)` per SPEC §9 incl. v2 and §15.6
(lines 529–590, 1584–1604): exact text with em/en dashes, `Window: empty`,
`No incidents.`, merged incidents omitted, counts from `summary`, and the audit block
lines (Team/Policy/Responder/Acknowledged by with ` (out-of-band)`/State/Notifications/
Timeline or `- Timeline: none`) only when `audit` is not None.

**Blocked by:** 01

**Status:** ready-for-agent

**Files:** incidentkit/metrics.py, incidentkit/report.py

**Context:**
`SPEC.md:410  §7 percentile(values, p), mtta(incidents), mttr(incidents), summary(incidents), by_service(incidents), availability(incidents, service, start, end)`
`SPEC.md:437  §7 v2 changes`
`SPEC.md:1190 §13.6 escalated / out_of_band_acks`
`SPEC.md:529  §9 render text template`
`SPEC.md:578  §9 v2 changes`
`SPEC.md:1584 §15.6 report timeline block`
`SPEC.md:1641 §15.7 expected summary dict and incident block`
`incidentkit/correlate.py  Incident (fields + severity/title/services/is_open properties); suspect_changes(incident, timeline, *, lookback) (ticket 09)`
`incidentkit/lifecycle.py  AuditLog.for_incident(id) -> list[AuditEntry]; AuditEntry.line() (ticket 08)`
`incidentkit/timeutil.py  format_ts(ms); format_duration(ms)`
`incidentkit/timeline.py  Timeline iteration (first/last ts)`

- [ ] `percentile([1..10], 90) == 9`; `percentile([], 50) is None`; `p=101` raises ValueError; float p uses exact arithmetic
- [ ] `summary` of one hand-built incident has 9 keys ending `"escalated": 0, "out_of_band_acks": 0`; merged incidents are not counted
- [ ] the §15.7 summary dict is reproduced from a hand-built incident with those fields and notifications
- [ ] `availability` merges overlapping HIGH+ intervals, clips to range, treats open incidents as covering to `end`, and honours `severity_override`
- [ ] `render` with no incidents and empty timeline gives `Window: empty` and ends `## Incidents\n\nNo incidents.\n`
- [ ] `render(..., audit=log)` reproduces the §15.7 incident block exactly; with `audit=None` the audit lines are absent; an incident with no entries shows `- Timeline: none`

# 07: report — Markdown render

**What to build:** Implement `render(incidents, timeline, *, title="Incident report", lookback=3_600_000) -> str` in `incidentkit/report.py` producing exactly the text in SPEC §9: header, Window line (`format_ts(first) – format_ts(last)` with EN DASH U+2013, or `Window: empty`), `Incidents: {count} ({open} open)`, Summary severity table (critical..info), `MTTA:`/`MTTR:` via `format_duration` or `n/a`, `## Incidents`, then per incident a blank line and the block with `### {id} — {title}` (EM DASH U+2014), Acknowledged `—` when None, Resolved `open`, Duration `ongoing`, Suspect changes joined with `"; "` or `none`. No incidents → blank line + `No incidents.`. Every line ends `\n`, result ends with exactly one `\n`. Use `metrics.summary`/`mtta`/`mttr` and `correlate.suspect_changes`.

**Blocked by:** 01

**Status:** ready-for-agent

**Files:** incidentkit/report.py

**Context:**
`SPEC.md:373  §9 Report  — exact template`
`incidentkit/report.py  render(incidents, timeline, *, title="Incident report", lookback=3_600_000) -> str  — stub`
`incidentkit/metrics.py  summary(incidents) -> dict; mtta(incidents); mttr(incidents)  — ticket 05`
`incidentkit/correlate.py  suspect_changes(incident, timeline, *, lookback) -> list[Event]  — ticket 04`
`incidentkit/correlate.py  Incident.id/.title/.severity/.services/.opened_at/.acked_at/.resolved_at/.alerts`
`incidentkit/timeutil.py  format_ts(ms) -> str; format_duration(ms) -> str  — ticket 02`
`incidentkit/timeline.py  Timeline: len(), t[0], t[-1]`

- [ ] Empty timeline and no incidents render `Window: empty`, `Incidents: 0 (0 open)`, all-zero table, `MTTA: n/a`, `MTTR: n/a`, and end `## Incidents\n\nNo incidents.\n`.
- [ ] An open, unacked incident renders `- Acknowledged: —`, `- Resolved: open`, `- Duration: ongoing`.
- [ ] A resolved incident with suspect deploys renders `- Suspect changes: svc: msg (ts); svc2: msg2 (ts)` in suspect_changes order, honouring `lookback`.
- [ ] Output uses U+2013 in the Window line and U+2014 in incident headers, and ends with exactly one newline.

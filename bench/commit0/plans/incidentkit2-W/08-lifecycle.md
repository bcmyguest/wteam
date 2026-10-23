# 08: Incident lifecycle, note commands, audit log

**What to build:** Implement `incidentkit/lifecycle.py` per SPEC §15.1–15.4 (lines
1381–1507): state constants; the transition functions `apply_open`, `apply_ack`,
`apply_mitigate`, `apply_resolve`, `apply_reopen`, `apply_severity`, `apply_merge` with
the legality table (illegal → `IllegalTransition(incident.id, incident.state, action)`,
nothing changed and nothing logged; reopen beyond window → `ReopenWindowError`), their
effects (first-ack fields, `responded_at`, merge sorting/min rules, `merged_into`,
`state` updates) and audit entries with exact `detail` text (ack `by=... out-of-band`,
reopen `after=<format_duration>`, severity `old->new` of the `severity` property, merge
then absorb); `parse_command` (`/sev`, `/mitigate`, `/merge ... into ...`, `CommandError`
for everything else starting with `/`); `AuditLog` (`append` rejects decreasing ts with
`ValueError`, `entries` tuple, len/iter, `for_incident`, `render`). Operates on the
tracer's `Incident` dataclass (import it from `correlate` lazily or accept it duck-typed
— do not create an import cycle, since `correlate` imports `lifecycle`).

**Blocked by:** 01

**Status:** ready-for-agent

**Files:** incidentkit/lifecycle.py

**Context:**
`SPEC.md:1385 §15.1 TRIGGERED, ACKNOWLEDGED, MITIGATED, RESOLVED, MERGED, ACTIVE_STATES`
`SPEC.md:1399 §15.2 transition table: apply_open(incident, log=None), apply_ack(incident, ts, log=None, *, by=None, on_call=None), apply_mitigate(incident, ts, log=None), apply_resolve(incident, ts, log=None), apply_reopen(incident, ts, log=None, *, reopen_window), apply_severity(incident, ts, severity, log=None), apply_merge(source, target, ts, log=None)`
`SPEC.md:1409 apply_ack / apply_reopen / apply_merge detailed effects`
`SPEC.md:1445 §15.3 Command(name, incident, arg); parse_command(text) -> Command | None`
`SPEC.md:1473 §15.4 AuditEntry(ts, incident, action, from_state, to_state, detail="").line(); AuditLog; entry table`
`SPEC.md:1629 §15.7 expected audit.render() text`
`incidentkit/correlate.py  Incident dataclass (fields of §6 v2) and .severity property`
`incidentkit/errors.py  IllegalTransition(incident_id, state, action); ReopenWindowError; CommandError(message)`
`incidentkit/timeutil.py  format_ts(ms); format_duration(ms)`
`incidentkit/model.py  Severity; Event.sort_key()`

- [ ] driving one Incident through open, ack(by="bob", on_call={"dan","lead"}), severity HIGH, mitigate, resolve, reopen(+3m, window 10m), resolve renders exactly the seven §15.7 audit lines
- [ ] an illegal transition (e.g. ack twice, mitigate from mitigated, resolve when resolved) raises IllegalTransition with `(id, state, action)` and appends nothing
- [ ] `apply_reopen` at exactly `resolved_at + reopen_window` succeeds; one ms later raises ReopenWindowError and leaves the incident unchanged
- [ ] `apply_ack` after a reopen keeps `acked_at`, `acked_by`, `out_of_band_ack`; ack from mitigated with `acked_at None` stays `mitigated`
- [ ] `apply_merge` sorts alerts stably (target first on ties), lowers `opened_at`, takes the earlier ack trio, mins `mitigated_at`/`responded_at`, sets source `merged`/`merged_into`, and logs merge then absorb; different teams or same id raise IllegalTransition
- [ ] every `parse_command` example of §15.3 holds; `/ack INC-0001`, `/`, `/merge INC-1 into INC-0002`, `/merge INC-0001 to INC-0002`, `/merge INC-0001 into INC-0001` raise CommandError
- [ ] `AuditLog.append` with a smaller ts than the last raises ValueError; equal ts is allowed; `render()` of an empty log is `""`

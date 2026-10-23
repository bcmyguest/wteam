# 09: Correlation (v2 algorithm) and suspect changes

**What to build:** Implement `correlate` and `suspect_changes` in
`incidentkit/correlate.py` per SPEC §6 (lines 296–408) and the complete v2 walk of
§15.5 (lines 1509–1582): routing alerts with `route(rules, e)` and team-eligible joining
(fingerprint → lowest id; service within window → most recent latest alert, tie lowest
id), reopen candidates when `reopen_window` is set (fingerprint filter, else service;
greatest `resolved_at`, tie lowest id; `apply_reopen` then append), new incidents with
sequential ids `INC-0001`…, team/policy/responder and `apply_open`; ack/resolve
targeting ignoring teams (fingerprint → lowest-id open incident, no fallback; else
highest-id open incident with the service) via `apply_ack` (with `by` and `on_call`) and
`apply_resolve`, catching `IllegalTransition`; note commands via `parse_command`
(sev/mitigate/merge, ignore errors and unknown ids); after the walk, `notifications =
escalate(...)` for every incident. `schedules`/`policies` `None` mean `{}`; negative
`reopen_window` → `ValueError`; audit receives entries in walk order. Without the v2
keywords and without note commands, the result equals v1. `suspect_changes` per §6
(deploys of incident services in `[opened_at - lookback, opened_at]`, most recent first,
ties later-in-timeline first). Do not change the `Incident` dataclass fields from 01.

**Blocked by:** 01, 08

**Status:** ready-for-agent

**Files:** incidentkit/correlate.py

**Context:**
`SPEC.md:318  §6 v1 algorithm (reference for unchanged behaviour)`
`SPEC.md:342  suspect_changes(incident, timeline, *, lookback=3_600_000) -> list[Event]`
`SPEC.md:351  §6 v2 changes to correlation`
`SPEC.md:912  §11.7 teams in correlation + differing example`
`SPEC.md:1083 §12.8 responders and acks in correlation`
`SPEC.md:1158 §13.3 responder set at creation`
`SPEC.md:1509 §15.5 full algorithm A–E + reopen example`
`SPEC.md:1606 §15.7 worked example`
`incidentkit/routing.py  route(ruleset, event) -> Route(team, policy, rule)`
`incidentkit/escalation.py  on_call_users(policy, schedules, ts); responder(policy, schedules, ts); escalate(incident, policies, schedules)`
`incidentkit/lifecycle.py  apply_open, apply_ack(..., by=, on_call=), apply_resolve, apply_reopen(..., reopen_window=), apply_severity, apply_mitigate, apply_merge; parse_command`
`incidentkit/errors.py  IllegalTransition; CommandError`
`incidentkit/model.py  Severity.parse; Event.attr("by")`
`incidentkit/timeline.py  Timeline iteration in sort order`

- [ ] v1 behaviour: fingerprint join, service join within window, window expiry opening a new incident, ack/resolve targeting, ack after resolve ignored, ids in order
- [ ] §11.7 example: with the §11.6 rules two incidents (`compliance`/None and `ops`/`ops-page`); with `rules=None` one incident with 2 alerts
- [ ] §15.5 reopen example: `reopen_window=300_000` → one open incident with `reopen_count == 1`; `30_000` → v1 result; `-1` → ValueError
- [ ] §15.3 example: note `/sev INC-0001 critical` makes severity CRITICAL with title "disk"
- [ ] a `/merge INC-0002 into INC-0001` note leaves INC-0002 in the result with `merged_into == "INC-0001"` and moves its alerts into INC-0001
- [ ] §15.7 worked example (rules, schedules, policies, reopen 10m) gives responder `dan`, `acked_by "bob"`, `out_of_band_ack True`, state `resolved`, 3 notifications, and the seven audit lines
- [ ] `suspect_changes` orders deploys most recent first with ties later-in-timeline first and respects the inclusive range

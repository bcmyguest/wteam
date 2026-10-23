# 04: correlate — group alerts into incidents, suspect changes

**What to build:** Implement `correlate(timeline, *, window=300_000)` and `suspect_changes(incident, timeline, *, lookback=3_600_000)` in `incidentkit/correlate.py` per SPEC §6. Keep the `Incident` dataclass from 01 (fix its properties if they disagree with the spec). Walk events in timeline order considering only alert/ack/resolve; maintain open incidents; alert join rules: (1) fingerprint match among open incidents, earliest opened; (2) same service and latest alert `ts >= alert.ts - window`, pick most recent latest alert, tie earliest opened; (3) new incident `INC-0001`… numbered in opening order. Ack/resolve target: fingerprint match (earliest opened) else most recently opened open incident including the service; none → ignored. Ack sets `acked_at` only if None; resolve sets `resolved_at` and closes. Return all incidents in id order. `suspect_changes`: deploy events with service in `incident.services` and ts in `[opened_at - lookback, opened_at]`, most recent first, ties later-in-timeline first.

**Blocked by:** 01

**Status:** ready-for-agent

**Files:** incidentkit/correlate.py

**Context:**
`SPEC.md:226  §6 Correlation  — normative rules`
`incidentkit/correlate.py  @dataclass Incident(id, opened_at, acked_at, resolved_at, alerts) + services/fingerprints/severity/title/is_open  — from 01`
`incidentkit/correlate.py  correlate(timeline, *, window=300_000) -> list[Incident]  — stub`
`incidentkit/correlate.py  suspect_changes(incident, timeline, *, lookback=3_600_000) -> list[Event]  — stub`
`incidentkit/timeline.py  class Timeline  — iterable in sort_key order (01)`
`incidentkit/model.py  Event(ts, kind, service, severity, message, source, fingerprint, attrs)`

- [ ] Two alerts for the same service within the window join one incident; beyond the window open a second incident (`INC-0002`).
- [ ] An alert with a fingerprint joins the open incident holding that fingerprint even outside the window and for a different service.
- [ ] Ack sets `acked_at` once (a second ack doesn't change it); resolve closes the incident so a later alert opens a new one; resolve without ack leaves `acked_at` None.
- [ ] Ack/resolve with no target are ignored; non-alert kinds (deploy, log, note) are ignored.
- [ ] `suspect_changes` returns only deploys of the incident's services within `[opened_at - lookback, opened_at]` inclusive, most recent first, equal-ts ties in reverse timeline order.

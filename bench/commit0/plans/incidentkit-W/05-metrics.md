# 05: metrics — percentile, MTTA/MTTR, summary, by_service, availability

**What to build:** Implement SPEC §7 in `incidentkit/metrics.py`: `percentile(values, p)` nearest-rank with exact arithmetic (`math.ceil(p*n/100)` for int `p`, `fractions.Fraction(p)` for floats), rank at least 1, empty → None, `p` outside [0,100] → `ValueError`; `mtta`/`mttr` floor-mean or None; `summary` dict with keys in spec order and `by_severity` always holding all five keys `critical, high, medium, low, info` in that order, `p90_ttr_ms` = percentile 90 of resolved durations; `by_service` keyed in sorted order, each incident counted once per service, `mttr_ms` per service; `availability` = fraction of `[start, end)` not covered by incidents including the service with severity >= HIGH, intervals `[opened_at, resolved_at or end)` clipped and merged, `round(x, 6)`, `end <= start` → `ValueError`.

**Blocked by:** 01

**Status:** ready-for-agent

**Files:** incidentkit/metrics.py

**Context:**
`SPEC.md:275  §7 Metrics  — normative rules and summary dict shape`
`incidentkit/metrics.py  percentile(values, p) / mtta(incidents) / mttr(incidents) / summary(incidents) / by_service(incidents) / availability(incidents, service, start, end)  — stubs from 01`
`incidentkit/correlate.py  Incident(id, opened_at, acked_at, resolved_at, alerts); .services, .severity, .is_open  — from 01`
`incidentkit/model.py  Severity IntEnum INFO..CRITICAL`

- [ ] `percentile([1,2,3,4,5], 90) == 5`, `percentile([..], 0)` returns the smallest, empty → None, `p=101` and `p=-1` raise `ValueError`; float `p` uses exact rank.
- [ ] `mtta`/`mttr` floor the mean; None when no qualifying incident.
- [ ] `summary` returns exactly the spec keys, counts open/resolved, all five severities (zero when absent) in order.
- [ ] `by_service` keys are sorted; an incident on two services counts for both.
- [ ] `availability` merges overlapping HIGH+ intervals, ignores lower severities and other services, treats open incidents as running to `end`, clips to range, rounds to 6 places; `end <= start` raises.

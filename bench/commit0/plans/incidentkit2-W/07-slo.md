# 07: SLOs and burn-rate alerts

**What to build:** Implement `incidentkit/slo.py` per SPEC §14 (lines 1229–1379) and the
config-file rules of SPEC §0, using `fractions.Fraction` only: `load_slos(text)` with
validation (`SloError`; name regex, unique, objective a string strictly in (0,100) —
JSON numbers rejected, `window_days` 1–365, `bad_severity` default high, `burn_alerts`
default two rules, burn rule durations positive multiples of 60 000 with `short < long`,
`DurationParseError` → `SloError`, factor string > 0, severity default critical);
`budget_fraction`, `bad_minutes`, `burn_rate` (`ValueError` for `minutes <= 0`),
`status` (window excludes the minute containing `at`), `format_fixed` (exact round,
ties to even), `format_percent`, burn-rule label and factor formatting,
`burn_alerts(slo, timeline, start, end)` (evaluation instants from ceil(start) minute,
alert on rising edge, resolve on falling edge, previous condition computed even before
start, event fields and messages exactly as specified; `ValueError` when `end <= start`)
and `feed(timeline, slos, start, end)` (new Timeline via `add`, original unchanged, all
alerts computed from the original). Keep the tracer's dataclasses.

**Blocked by:** 01

**Status:** ready-for-agent

**Files:** incidentkit/slo.py

**Context:**
`SPEC.md:1236 §14.1 load_slos(text) -> list[Slo]; BurnRule, Slo; budget_fraction(slo)`
`SPEC.md:1267 §14.2 bad_minutes(slo, timeline) -> set[int]`
`SPEC.md:1278 §14.3 burn_rate(slo, timeline, end_minute, minutes) -> Fraction`
`SPEC.md:1286 §14.4 status(slo, timeline, at) -> SloStatus`
`SPEC.md:1298 §14.5 format_fixed(value, places=2), format_percent(value), label, factor text`
`SPEC.md:1313 §14.6 burn_alerts(slo, timeline, start, end) -> list[Event]; feed(timeline, slos, start, end) -> Timeline`
`SPEC.md:1353 §14.7 worked example`
`incidentkit/timeline.py  Timeline(events=()); Timeline.add(events); iteration`
`incidentkit/timeutil.py  parse_duration(text); format_duration(ms)`
`incidentkit/model.py  Event(...); Severity.parse`
`incidentkit/errors.py  SloError(message); DurationParseError`

- [ ] the §14.7 `burn_alerts` call returns exactly the two listed Events
- [ ] the §14.7 `status` gives total 43200, bad 1, good 43199, allowed_bad 216/5, remaining 211/216, `format_percent == "97.69"`; 3 bad minutes give 67/72 → "93.06"
- [ ] every `format_fixed` example of §14.5 holds; factor 14.4 → "14.4", 6 → "6", 2.25 → "2.2"; a `"long": "60m"` rule has label `1h/5m`
- [ ] `burn_rate` with one bad minute in 60 at objective 99.9 is `Fraction(50, 3)`; `minutes=0` raises ValueError
- [ ] `feed` returns a new Timeline with original events then burn alerts; the input timeline is unchanged; synthetic alerts never count as bad minutes
- [ ] `"objective": 99.9` (number), `"objective": "100"`, `window_days: 0`, `short >= long`, `"long": "90s"` and `"long": "xx"` each raise SloError

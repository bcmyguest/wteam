# 05: On-call schedules

**What to build:** Implement `incidentkit/oncall.py` per SPEC §12.1–12.7 (lines
936–1081) and the config-file rules of SPEC §0 (lines 40–47): `load_schedules(text)`
with full validation (`ScheduleError` for bad JSON, non-object top level, non-list
`"schedules"`, every key rule of the schedule/layer/window/override tables, booleans
rejected as integers, `handoff_day` requiring `rotation_days % 7 == 0`, `"24:00"` only as
window end, `start == end` window error, override `end <= start`, override times parsed
with the schedule's zone and `TimeParseError` → `ScheduleError`); local time
(§12.2), rotation epoch/period (§12.3), restrictions incl. wrap past midnight (§12.4),
`who_is_on_call(schedule, ts)` (§12.5), `final_schedule(schedule, start, end)` (§12.6,
evaluated at all boundaries, merged maximal segments, gaps where nobody is on call,
`ValueError` for `end <= start`). Keep the tracer's `Schedule` field names; layer/window/
override types are free.

**Blocked by:** 01

**Status:** ready-for-agent

**Files:** incidentkit/oncall.py

**Context:**
`SPEC.md:40   config file rules (shared JSON, missing key = empty)`
`SPEC.md:938  §12.1 load_schedules(text) -> dict[str, Schedule]; key tables`
`SPEC.md:977  Schedule(name, tz_offset_ms, layers, overrides)`
`SPEC.md:981  §12.2 local time formulas`
`SPEC.md:993  §12.3 rotation epoch E and period P`
`SPEC.md:1009 §12.4 restriction windows`
`SPEC.md:1022 §12.5 who_is_on_call(schedule, ts) -> str | None`
`SPEC.md:1030 §12.6 final_schedule(schedule, start, end) -> list[tuple[int, int, str]]`
`SPEC.md:1041 §12.7 worked example`
`incidentkit/errors.py  ScheduleError(message); TimeParseError`
`incidentkit/timeutil.py  parse_ts(text, *, default_tz=...) -> int; zone-offset helper from ticket 01`

- [ ] the §12.7 config loads; `weekly` epoch is 1709535600000 and the override covers [1710237600000, 1710241200000)
- [ ] every row of the §12.7 who_is_on_call table gives the listed user
- [ ] `final_schedule(payments, 1710201600000, 1710288000000)` equals the five §12.7 tuples
- [ ] a window `fri 22:00–06:00` is active Friday 23:00 and Saturday 05:00 local, not Friday 03:00
- [ ] invalid JSON, `"schedules": {}`, duplicate names, `rotation_days: true`, `handoff_day` with rotation 3, `"24:00"` as start, and a bad override timestamp each raise ScheduleError; a config without `"schedules"` returns `{}`
- [ ] `final_schedule` with `end <= start` raises ValueError

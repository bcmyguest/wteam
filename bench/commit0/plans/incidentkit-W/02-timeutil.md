# 02: timeutil — parse/format timestamps and durations

**What to build:** Replace the stubs in `incidentkit/timeutil.py` with full SPEC §2 behaviour: `parse_ts` (epoch 1–10 digits = seconds, 13 digits = ms, other lengths error; ISO with `T` or one space, optional 1–9 digit fraction truncated to ms, zone `Z`/`z`/`±HH:MM` or `default_tz`; syslog `Mon DD HH:MM:SS` with case-insensitive month, optional extra space before 1-digit day, year from `default_year` else error), full range validation (leap years, zone ranges) raising `TimeParseError`; `parse_duration` (units d/h/m/s/ms in descending order, each at most once, leading `-`, no inner whitespace, `DurationParseError` on every bad form); `format_ts` (`YYYY-MM-DDTHH:MM:SS.mmmZ`); `format_duration` (non-zero components, `0` → `0s`, negatives prefixed `-`). Must handle negative ms in `format_ts` (pre-1970) correctly via datetime arithmetic. Validate `default_tz` itself with the same zone rule.

**Blocked by:** 01

**Status:** ready-for-agent

**Files:** incidentkit/timeutil.py

**Context:**
`SPEC.md:38   §2 Time  — normative rules and examples`
`incidentkit/timeutil.py  parse_ts(text, *, default_year=None, default_tz="+00:00") -> int  — stub from 01`
`incidentkit/timeutil.py  parse_duration(text) -> int  — stub from 01`
`incidentkit/timeutil.py  format_ts(ms) -> str  — stub from 01`
`incidentkit/timeutil.py  format_duration(ms) -> str  — stub from 01`
`incidentkit/errors.py  class TimeParseError(ValueError), class DurationParseError(ValueError)`

- [ ] All five `parse_ts` examples in SPEC §2 hold.
- [ ] `parse_ts("12345678901")` (11 digits) and `"Feb 30 00:00:00"` (with year) and `"2023-02-29T00:00:00Z"` raise `TimeParseError`; `"2024-02-29T00:00:00Z"` parses.
- [ ] Syslog form without `default_year` raises `TimeParseError`; `"mar  3 01:02:03"` parses case-insensitively.
- [ ] Fractions are truncated (`.9999` → 999 ms); 10+ fraction digits, `+24:00`, `+01:60` are errors.
- [ ] `parse_duration`: `"1h30m"`→5400000, `"90s"`→90000, `"1s500ms"`→1500, `"0s"`→0, `"-2h"`→-7200000; `""`, `"90"`, `"1m1h"`, `"1h1h"`, `"1x"`, `"h"`, `"1h 30m"` raise `DurationParseError`.
- [ ] `format_ts(1709294400000) == "2024-03-01T12:00:00.000Z"`; `format_duration` examples in SPEC hold, including `0`→`"0s"` and negatives.

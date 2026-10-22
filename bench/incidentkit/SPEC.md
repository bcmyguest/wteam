# incidentkit — specification

`incidentkit` is a pure-Python (3.11+, standard library only) toolkit for incident
data: it parses alert, log and change records, merges them into one timeline, groups
alerts into incidents, computes response metrics, answers a small query language and
renders a Markdown report.

Everything below is normative. Names in `code` are the exact public names; a hidden test
suite imports them from the modules given. Where a rule says "error", the named exception
is raised unless the section says errors are collected instead.

## 0. Package layout

```
incidentkit/__init__.py      re-exports nothing in particular (may be empty)
incidentkit/errors.py        exception classes
incidentkit/timeutil.py      timestamps and durations
incidentkit/model.py         Severity, Event
incidentkit/parsers.py       the three input formats
incidentkit/timeline.py      Timeline
incidentkit/correlate.py     Incident, correlate, suspect_changes
incidentkit/metrics.py       percentile, mtta, mttr, summary, by_service, availability
incidentkit/query.py         the query language
incidentkit/report.py        Markdown report
incidentkit/__main__.py      command line (`python -m incidentkit`)
```

## 1. Errors (`incidentkit/errors.py`)

All subclass `ValueError`:

- `TimeParseError`
- `DurationParseError`
- `QueryError` — has an attribute `pos: int`, the 0-based character offset in the query
  string where the offending token starts (for "unexpected end of query", `pos` is the
  length of the query string).

## 2. Time (`incidentkit/timeutil.py`)

All instants are **integers: milliseconds since the Unix epoch, UTC** ("ms").

### `parse_ts(text, *, default_year=None, default_tz="+00:00") -> int`

`text` is stripped of surrounding whitespace, then matched against these forms in order:

1. **Epoch**: only ASCII digits. 1–10 digits: seconds (`× 1000`). Exactly 13 digits:
   milliseconds. Any other length: `TimeParseError`.
2. **ISO**: `YYYY-MM-DD` + (`T` or one space) + `HH:MM:SS`, optionally `.` followed by
   1–9 digits (fraction of a second, **truncated** to milliseconds, never rounded),
   optionally followed by a zone: `Z`, `z` or `±HH:MM`. No zone means `default_tz`.
3. **Syslog**: `Mon DD HH:MM:SS` where `Mon` is an English 3-letter month abbreviation
   (case-insensitive: `Jan` … `Dec`) and `DD` is 1–2 digits, possibly preceded by an extra
   space (`Mar  3`). Exactly one space separates the other parts. The year is
   `default_year`; if `default_year` is `None`: `TimeParseError`. The zone is `default_tz`.

`default_tz` is `Z` or `±HH:MM`. Fields must be in range (month 1–12, day valid for the
month including leap years, hour 0–23, minute 0–59, second 0–59, zone hours 0–23 and
minutes 0–59), else `TimeParseError`. Anything else is a `TimeParseError`.

Examples: `parse_ts("2024-03-01T12:00:00Z") == 1709294400000`;
`parse_ts("2024-03-01 13:00:00+01:00") == 1709294400000`;
`parse_ts("1709294400") == 1709294400000`; `parse_ts("2024-03-01T12:00:00.9999Z") == 1709294400999`;
`parse_ts("Mar  1 12:00:00", default_year=2024) == 1709294400000`.

### `parse_duration(text) -> int`

Returns ms. Grammar: one or more `<digits><unit>` parts with no whitespace anywhere
(surrounding whitespace is stripped first). Units: `d` (86 400 000), `h` (3 600 000),
`m` (60 000), `s` (1000), `ms` (1). Units must appear in this descending order and each at
most once. A leading `-` negates the whole duration (`-2h`). Examples: `"1h30m"` → 5 400 000;
`"90s"` → 90 000; `"1s500ms"` → 1500; `"0s"` → 0. Errors (`DurationParseError`): empty
text, unknown unit, units out of order or repeated, a unit with no digits, digits with no
unit (`"90"`), whitespace inside.

### `format_ts(ms) -> str`

`YYYY-MM-DDTHH:MM:SS.mmmZ` in UTC, always with exactly three fraction digits.
`format_ts(1709294400000) == "2024-03-01T12:00:00.000Z"`.

### `format_duration(ms) -> str`

Concatenate non-zero components from largest to smallest: `<n>d`, `<n>h`, `<n>m`, `<n>s`,
`<n>ms`. `0` → `"0s"`. Negative → `"-"` + format of the absolute value. Examples:
5 400 000 → `"1h30m"`; 1500 → `"1s500ms"`; 86 400 000 → `"1d"`; 90 061 001 → `"1d1h1m1s1ms"`.

## 3. Model (`incidentkit/model.py`)

### `Severity`

An `enum.IntEnum`: `INFO = 0`, `LOW = 1`, `MEDIUM = 2`, `HIGH = 3`, `CRITICAL = 4`.

`Severity.parse(text) -> Severity` (a classmethod or staticmethod). Case-insensitive,
surrounding whitespace stripped. Accepts each member name (`info`, `low`, `medium`,
`high`, `critical`) and these aliases:

| alias | severity |
|---|---|
| `sev1`, `p1`, `crit`, `fatal`, `emerg`, `alert` | CRITICAL |
| `sev2`, `p2`, `error`, `err` | HIGH |
| `sev3`, `p3`, `warning`, `warn` | MEDIUM |
| `sev4`, `p4`, `notice` | LOW |
| `sev5`, `p5`, `debug` | INFO |

Anything else: `ValueError`.

`str(sev)` is not specified; use `sev.name.lower()` where text is needed (the report
does).

### `Event`

A frozen dataclass with fields, in this order:

| field | type | meaning |
|---|---|---|
| `ts` | `int` | instant, ms |
| `kind` | `str` | one of `deploy`, `log`, `alert`, `ack`, `resolve`, `note` |
| `service` | `str` | |
| `severity` | `Severity` | |
| `message` | `str` | |
| `source` | `str` | input format: `alerts`, `syslog`, `changes`, or anything for hand-built events |
| `fingerprint` | `str \| None` | default `None` |
| `attrs` | `tuple[tuple[str, str], ...]` | default `()`; sorted by key, keys unique |

Methods:

- `attr(key, default=None)` — the value for `key` in `attrs`, else `default`.
- `sort_key()` — `(ts, KIND_ORDER[kind], service, message)` where `KIND_ORDER` is a
  module-level dict `{"deploy": 0, "log": 1, "alert": 2, "ack": 3, "resolve": 4, "note": 5}`.

The constructor does not validate. Two events with equal fields are equal and hash equal.

## 4. Parsers (`incidentkit/parsers.py`)

Each parser takes the full file text and returns a `ParseResult`, a dataclass with
`events: list[Event]` (in input order) and `errors: list[ParseError]`. `ParseError` is a
dataclass `(line: int, reason: str)`; `line` is 1-based. A bad record is skipped and
recorded; parsers never raise for a bad record. Blank lines (only whitespace) are ignored
and are not errors. `reason` is free text.

### 4.1 `parse_alerts(text) -> ParseResult` (JSON Lines, source `"alerts"`)

Each non-blank line is a JSON object with:

| key | required | meaning |
|---|---|---|
| `at` | yes | a string accepted by `parse_ts` (no default year), or a JSON integer read with the epoch digit rule of `parse_ts` (its decimal digits) |
| `svc` | yes | non-empty string → `service` |
| `type` | yes | `"trigger"` → kind `alert`; `"ack"` → `ack`; `"resolve"` → `resolve` |
| `sev` | no | string for `Severity.parse`; absent → `MEDIUM` |
| `summary` | no | string → `message`; absent → `""` |
| `dedup_key` | no | string → `fingerprint`; absent or `""` → `None` |
| `labels` | no | object whose values are strings, numbers or booleans → `attrs`. Booleans become `"true"`/`"false"`; integers `str(n)`; floats `repr(x)`. Any other value type makes the record an error. |

Errors: invalid JSON, not an object, a required key missing, a wrong type, an unknown
`type`, a bad `at`, a bad `sev`. Extra keys are ignored. Booleans are not integers for `at`.

### 4.2 `parse_syslog(text, *, year, tz="+00:00") -> ParseResult` (source `"syslog"`)

Each non-blank line is:

```
<Mon DD HH:MM:SS> <host> <service>[<pid>]: <rest>
```

- The timestamp is the first 15 characters and is parsed by `parse_ts(..., default_year=…,
  default_tz=tz)`.
- `host` and `service` are runs of non-space characters; `service` may not contain `[`,
  `]` or `:`. `[<pid>]` (digits) is optional. Then a literal `: `.
- If `rest` starts with `<LEVEL>: ` where `<LEVEL>` is a run of letters and digits that
  `Severity.parse` accepts, that is the severity and the message is what follows;
  otherwise the severity is `INFO` and the message is all of `rest`. Trailing whitespace is
  stripped from the message.
- kind `log`; `fingerprint None`; attrs `host` and, when present, `pid`.

**Year rollover.** The year starts at `year`. If a line's timestamp (with the current
year) is more than 180 days earlier than the previous successfully parsed line's
timestamp, the current year increases by one for that line and all later lines (re-parse
with the new year). Rollover can happen several times.

### 4.3 `parse_changes(text) -> ParseResult` (CSV, source `"changes"`)

Read with the `csv` module (standard dialect, quoted fields allowed). The first record is
the header; names are matched case-insensitively after stripping whitespace. Required
columns: `time`, `service`, `change`. Optional: `author`, `ticket`, `risk`. If a required
column is missing, raise `ValueError` (the whole file is unusable). A file with no header
record returns an empty result.

For each later record (line number = the CSV line number where the record starts,
header = line 1): `time` by `parse_ts` (no default year); `service` non-empty after
stripping; `message` = `change` stripped; `risk` (`low` / `medium` / `high`,
case-insensitive) → `LOW` / `MEDIUM` / `HIGH`, empty or column absent → `INFO`, other →
error. kind `deploy`; attrs `author` and `ticket` when present and non-empty after
stripping. A record with the wrong number of fields is an error.

### 4.4 `load(path, *, year=None, tz="+00:00") -> ParseResult`

Reads the file as UTF-8 and dispatches on extension: `.jsonl` → `parse_alerts`, `.log`
→ `parse_syslog` (`year` required: `ValueError` if `None`), `.csv` → `parse_changes`.
Other extensions: `ValueError`.

## 5. Timeline (`incidentkit/timeline.py`)

`Timeline(events=())` holds events sorted by `Event.sort_key()`; the sort is stable, so
events with equal keys keep insertion order.

- `add(events)` — inserts an iterable of events, keeping the order rule (new events go
  after existing ones with an equal key).
- `len(t)`, iteration in order, `t[i]` → `Event`, `t[a:b]` → `list[Event]`.
- `events` property → a new `list` of the events in order.
- `services()` → sorted list of distinct services.
- `between(start, end)` → new `Timeline` with `start <= ts < end`.
- `filter(pred)` → new `Timeline` of events for which `pred(event)` is truthy.
- `dedupe(window)` → new `Timeline`. Walk in order; an event's **identity** is
  `(kind, service, fingerprint)` when `fingerprint` is not `None`, else
  `(kind, service, message)`. Drop the event if the most recently **kept** event with the
  same identity has `ts >= event.ts - window`; otherwise keep it.
  (With window 50, identities at ts 0, 40, 80: 40 is dropped, 80 is kept.)
- `gaps(min_gap)` → list of `(prev_ts, next_ts)` for each pair of consecutive events with
  `next_ts - prev_ts >= min_gap`, in order.
- `bucket(width, *, start=None)` → list of `(bucket_start, count)`. `start` defaults to
  the first event's `ts` floored to a multiple of `width` (floor division, works for
  negative). Buckets are `[start + k·width, start + (k+1)·width)` for `k = 0, 1, …` up to
  and including the bucket holding the last event; empty buckets are included; events
  before `start` are not counted. Empty timeline → `[]`. `width <= 0` → `ValueError`.

## 6. Correlation (`incidentkit/correlate.py`)

### `Incident`

A (mutable) dataclass:

| field | type |
|---|---|
| `id` | `str` — `"INC-0001"`, `"INC-0002"`, … in the order incidents open |
| `opened_at` | `int` |
| `acked_at` | `int \| None` |
| `resolved_at` | `int \| None` |
| `alerts` | `list[Event]` — alert events in order |

Properties:

- `services` → sorted tuple of the distinct services of its alerts.
- `fingerprints` → sorted tuple of the distinct non-`None` fingerprints of its alerts.
- `severity` → max severity of its alerts.
- `title` → message of the first alert (in order) whose severity equals `severity`.
- `is_open` → `resolved_at is None`.

### `correlate(timeline, *, window=300_000) -> list[Incident]`

Walk the timeline in order, considering only `alert`, `ack` and `resolve` events, with a
set of **open** incidents (not yet resolved):

- **alert** — find the incident to join:
  1. if the alert has a fingerprint and an open incident has that fingerprint among its
     alerts, join it (if several, the one opened earliest);
  2. else, among open incidents that include the alert's service and whose **latest
     alert** has `ts >= alert.ts - window`, join the one whose latest alert is most
     recent (tie: opened earliest);
  3. else open a new incident with `opened_at = alert.ts`.
- **ack / resolve** — find the target: if the event has a fingerprint, the open incident
  with that fingerprint (earliest opened if several); otherwise the most recently opened
  open incident that includes the event's service. No target: ignore the event. An ack
  sets `acked_at` if it is `None` (an ack after resolution cannot happen, since resolved
  incidents are not open). A resolve sets `resolved_at` and closes the incident; if
  `acked_at` is still `None` it stays `None`.

Returns all incidents, in id order.

### `suspect_changes(incident, timeline, *, lookback=3_600_000) -> list[Event]`

The `deploy` events whose service is among `incident.services` and whose `ts` is in
`[incident.opened_at - lookback, incident.opened_at]`, most recent first (ties: later in
timeline order first).

## 7. Metrics (`incidentkit/metrics.py`)

- `percentile(values, p)` — nearest rank: sort ascending, `rank = ceil(p / 100 · n)`,
  at least 1, return the value at that rank (1-based). Empty → `None`. `p` outside
  `[0, 100]` → `ValueError`. Compute the rank with exact arithmetic
  (`math.ceil(p * n / 100)` for integer `p`; use `fractions.Fraction(p)` for floats).
- `mtta(incidents)` — mean of `acked_at - opened_at` over incidents with `acked_at`,
  floored to an int (`//`). None qualifying → `None`.
- `mttr(incidents)` — same with `resolved_at`.
- `summary(incidents)` → dict:

  ```
  {"count": int, "open": int, "resolved": int,
   "by_severity": {"critical": n, "high": n, "medium": n, "low": n, "info": n},
   "mtta_ms": int | None, "mttr_ms": int | None, "p90_ttr_ms": int | None}
  ```

  `by_severity` always has all five keys in that order. `p90_ttr_ms` is
  `percentile` 90 of the resolved durations.
- `by_service(incidents)` → dict keyed by service (keys inserted in sorted order) with
  `{"count": n, "mttr_ms": int | None}`; an incident counts once for each of its services.
- `availability(incidents, service, start, end) -> float` — the fraction of `[start, end)`
  not covered by incidents that include `service` and have severity `>= HIGH`. An
  incident covers `[opened_at, resolved_at)` (or `[opened_at, end)` when open), clipped
  to the range; overlapping intervals are merged. Rounded with `round(x, 6)`.
  `end <= start` → `ValueError`.

## 8. Query language (`incidentkit/query.py`)

### `run(query, timeline)`

Returns a `list[Event]`, an `int`, or a `list[tuple[str, int]]`, depending on the last
stage. `compile(query)` returns an object with a `.run(timeline)` method giving the same
result; both raise `QueryError` for a bad query (at compile time — no partial results).

### Lexical rules

- Whitespace separates tokens and is otherwise ignored.
- Keywords, case-insensitive: `and`, `or`, `not`, `head`, `tail`, `sort`, `desc`,
  `count`, `by`, `dedupe`.
- Strings: double-quoted; `\"` and `\\` are the only escapes; any other backslash
  sequence or an unterminated string is an error.
- Integers: `-?[0-9]+`.
- Durations in stages: a run of non-space characters parsed by `parse_duration`.
- Identifiers: `[A-Za-z_][A-Za-z0-9_.-]*` (not a keyword).
- Operators: `=`, `!=`, `<`, `<=`, `>`, `>=`, `~`; also `(`, `)`, `|`.

### Grammar

```
query  := [expr] ("|" stage)*
expr   := and ("or" and)*
and    := unary ("and" unary)*
unary  := "not" unary | "(" expr ")" | cmp
cmp    := field op value
field  := "ts" | "kind" | "service" | "severity" | "message" | "source"
        | "fingerprint" | "attr." name          (one identifier token, e.g. attr.host)
value  := string | integer | identifier
stage  := "head" N | "tail" N | "sort" field ["desc"] | "dedupe" DURATION
        | "count" | "count" "by" field
```

`N` is a non-negative integer. Field names are case-sensitive and lowercase. An unknown
field, a missing piece, or a token where another is expected is a `QueryError` whose
`pos` is the offset of that token. `count` and `count by` must be the last stage
(otherwise `QueryError` at the next `|`).

### Comparison semantics

- `severity`: the value (identifier, string, or integer 0–4) is converted with
  `Severity.parse` (or `Severity(n)` for integers; out of range is an error) and compared
  by level. `~` is not allowed on `severity` or `ts`.
- `ts`: the value is a string parsed by `parse_ts` (no default year) or an integer taken
  as **ms** directly. Compared numerically.
- every other field: the value must be a string or identifier (an integer is an error);
  compared as strings (`<`, `>` lexicographic). `~` is case-insensitive substring
  containment (`value.lower() in field.lower()`).
- `fingerprint` of `None` and a missing `attr.<name>` are **missing**: every comparison
  with a missing field is false, including `!=`. (`not` then makes it true.)
- Precedence: `not` binds tightest, then `and`, then `or`.

### Stages

Applied left to right to the filtered events:

- `head N` / `tail N` — first / last `N` events.
- `sort FIELD [desc]` — stable sort by the field value; severity by level, ts
  numerically, others as strings; missing values sort before all present values
  (ascending) — with `desc` the order is fully reversed, so missing values come last;
  among equal keys the incoming order is kept in both directions.
- `dedupe DURATION` — as `Timeline.dedupe`.
- `count` — number of events (int).
- `count by FIELD` — list of `(value_as_string, count)` sorted by count descending then
  value ascending. Severity values are the lowercase name; ts values `str(ms)`; missing
  values are grouped under `"-"`.

An empty query (or only whitespace) returns all events.

## 9. Report (`incidentkit/report.py`)

`render(incidents, timeline, *, title="Incident report", lookback=3_600_000) -> str`
returns exactly the text below (`{…}` are substitutions; `—` is U+2014 EM DASH, `–` is
U+2013 EN DASH). Lines end with `\n`; the result ends with exactly one `\n`.

```
# {title}

Window: {format_ts(first ts)} – {format_ts(last ts)}
Incidents: {count} ({open} open)

## Summary

| Severity | Count |
|---|---|
| critical | {n} |
| high | {n} |
| medium | {n} |
| low | {n} |
| info | {n} |

MTTA: {format_duration(mtta) or n/a}
MTTR: {format_duration(mttr) or n/a}

## Incidents
```

then, for each incident in the order given, a blank line followed by:

```
### {id} — {title}
- Severity: {severity name lowercase}
- Services: {services joined with ", "}
- Opened: {format_ts(opened_at)}
- Acknowledged: {format_ts(acked_at) or —}
- Resolved: {format_ts(resolved_at) or open}
- Duration: {format_duration(resolved_at - opened_at) or ongoing}
- Alerts: {len(alerts)}
- Suspect changes: {suspects or none}
```

Suspects are `suspect_changes(incident, timeline, lookback=lookback)` each written
`{service}: {message} ({format_ts(ts)})`, joined with `"; "`.

If the timeline is empty the Window line is `Window: empty`. If there are no incidents
the report ends after `## Incidents` with the line `No incidents.` preceded by a blank
line. `n/a` and `open`, `ongoing`, `none` are literal.

## 10. Command line (`python -m incidentkit`)

```
python -m incidentkit report [--year Y] [--tz TZ] [--window DUR] [--dedupe DUR] [--title T] [--strict] FILE...
python -m incidentkit stats  [--year Y] [--tz TZ] [--window DUR] [--dedupe DUR] [--strict] FILE...
python -m incidentkit query  [--year Y] [--tz TZ] [--strict] QUERY FILE...
```

- Files are loaded with `load` (`--year` defaults to `None`, `--tz` to `+00:00`) and
  merged into one `Timeline`. Each parse error is printed to stderr as
  `{path}:{line}: {reason}` and the record skipped.
- `--dedupe DUR` applies `Timeline.dedupe` before correlation. `--window DUR` is the
  correlation window (default `5m`).
- `report` prints `render(correlate(...), timeline, title=...)` (no extra newline).
- `stats` prints `json.dumps(summary(incidents), indent=2)` followed by a newline.
- `query` prints, for an event list, one line per event:
  `{format_ts(ts)} {kind} {service} {severity name lowercase} {message}`; for an int, the
  number; for a `count by` result, `{value}\t{count}` per line.
- Exit status: `0` on success; `1` if `--strict` was given and any parse error occurred
  (output is still printed); `2` for a usage error, an unreadable file, a `ValueError`
  from `load`, an invalid duration or a `QueryError` — with a one-line message on stderr
  and nothing on stdout.

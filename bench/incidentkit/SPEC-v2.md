# incidentkit — specification (v2)

`incidentkit` is a pure-Python (3.11+, standard library only) toolkit for incident
data: it parses alert, log and change records, merges them into one timeline, groups
alerts into incidents, computes response metrics, answers a small query language and
renders a Markdown report.

Everything below is normative. Names in `code` are the exact public names; a hidden test
suite imports them from the modules given. Where a rule says "error", the named exception
is raised unless the section says errors are collected instead.

**Version 2.** Sections 0–10 are the v1 specification. Rules marked "(v2)" were added to
them for the new subsystems; sections 11–15 are new. A (v2) rule that changes v1
behaviour says so explicitly and gives an example that differs; every other v1 rule and
example still holds. A pointer such as "§13.4" names the normative rule; the pointing
text only summarizes it. "§N v2" means the (v2) additions inside section N (for example
"§6 v2" is the subsection "(v2) Changes to correlation").

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
incidentkit/routing.py       (v2) routing rules, §11
incidentkit/oncall.py        (v2) on-call schedules, §12
incidentkit/escalation.py    (v2) escalation policies, §13
incidentkit/slo.py           (v2) SLOs and burn-rate alerts, §14
incidentkit/lifecycle.py     (v2) incident states, note commands, audit log, §15
```

(v2) **Config file.** One JSON document holds on-call schedules, escalation policies
and SLOs under the top-level keys `"schedules"` (§12.1), `"policies"` (§13.1) and
`"slos"` (§14.1). Each loader receives the whole text and reads only its own key; other
top-level keys are ignored, and a missing key means an empty collection. If the text is
not valid JSON, or the top level is not an object, or the loader's key is present with a
value that is not a list, the loader raises its own error class (`ScheduleError`,
`PolicyError`, `SloError` respectively). Unknown keys inside the objects of these lists
are ignored.

(v2) **Rules file.** Routing rules (§11) live in a separate text file.

## 1. Errors (`incidentkit/errors.py`)

All subclass `ValueError`:

- `TimeParseError`
- `DurationParseError`
- `QueryError` — has an attribute `pos: int`, the 0-based character offset in the query
  string where the offending token starts (for "unexpected end of query", `pos` is the
  length of the query string).

(v2) Also in `errors.py`, all subclassing `ValueError`:

- `RoutingError` — constructed as `RoutingError(line, reason)`; attributes `line: int`
  (1-based line number in the rules file) and `reason: str`;
  `str(err) == f"line {line}: {reason}"`. See §11.4.
- `ScheduleError` — invalid schedule config, §12.1.
- `PolicyError` — invalid escalation-policy config, §13.1.
- `SloError` — invalid SLO config, §14.1.
- `CommandError` — a malformed note command, §15.3.
- `IllegalTransition` — constructed as `IllegalTransition(incident_id, state, action)`;
  attributes `incident_id: str`, `state: str`, `action: str` with those values. See §15.2.
- `ReopenWindowError` — a subclass of `IllegalTransition` (same constructor), §15.2.

`ScheduleError`, `PolicyError`, `SloError` and `CommandError` take one message string;
their message text is free.

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

(v2) `parse_ts` also reads schedule override instants, with `default_tz` set to the
schedule's zone (§12.1). `parse_duration` also reads SLO burn windows (§14.1) and the
CLI option `--reopen` (§10.1).

### `format_duration(ms) -> str`

Concatenate non-zero components from largest to smallest: `<n>d`, `<n>h`, `<n>m`, `<n>s`,
`<n>ms`. `0` → `"0s"`. Negative → `"-"` + format of the absolute value. Examples:
5 400 000 → `"1h30m"`; 1500 → `"1s500ms"`; 86 400 000 → `"1d"`; 90 061 001 → `"1d1h1m1s1ms"`.

(v2) `format_duration` renders SLO window labels (§14.5), so an SLO window written `"60m"`
appears as `"1h"` in fingerprints and messages, and the `after=` detail of a reopen audit
entry (§15.4).

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

(v2) `Severity.parse` (aliases included) also reads the routing `severity` matcher
(§11.3), the level of a `/sev` note command (§15.3), and the SLO keys `bad_severity` and
`severity` (§14.1).

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
  (v2) Because `note` sorts last, a note command (§15.3) with the same `ts` as the alert
  that opens an incident is processed after that alert and can refer to the new incident
  (§15.5).

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
| `by` | no | (v2) read only when `type` is `"ack"`: a non-empty string → the attr `by` (it replaces a label named `by`). On an ack, any other value (non-string or `""`) makes the record an error. For other types the key is ignored. |

Errors: invalid JSON, not an object, a required key missing, a wrong type, an unknown
`type`, a bad `at`, a bad `sev`. Extra keys are ignored. Booleans are not integers for `at`.

(v2) `type` also accepts `"note"` → kind `note` (all other keys as for the other types;
`summary` becomes the message, which may be a note command, §15.3). **This changes v1
behaviour:** in v1 the line
`{"at": 0, "svc": "api", "type": "note", "summary": "/mitigate INC-0001"}` was an error
(unknown `type`); in v2 it yields `Event(0, "note", "api", Severity.MEDIUM,
"/mitigate INC-0001", "alerts")`.

(v2) The `by` attr of an ack is used by correlation for `acked_by` and out-of-band acks
(§12.8, §15.2), and can be queried as `attr.by` (§8).

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

(v2) Syslog `log` events are the input of SLO bad-minute counting (§14.2).

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

(v2) `slo.feed` (§14.6) builds a new timeline with `add`, so a synthetic SLO event whose
sort key equals an existing event's key comes after it.

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

(v2) This algorithm is extended by teams (§11.7), reopening and note commands (§15.5),
which replaces it as a whole; see "(v2) Changes to correlation" below.

### `suspect_changes(incident, timeline, *, lookback=3_600_000) -> list[Event]`

The `deploy` events whose service is among `incident.services` and whose `ts` is in
`[incident.opened_at - lookback, incident.opened_at]`, most recent first (ties: later in
timeline order first).

(v2) After a merge (§15.2) `incident.services` and `incident.opened_at` are those of the
merged target, so its suspects can include changes to the absorbed incident's services.

### (v2) Changes to correlation

Called without the new keyword arguments on a timeline that contains no note commands
(§15.3), `correlate` returns the same incidents as v1, with the same `id`, `opened_at`,
`acked_at`, `resolved_at` and `alerts`. (The new fields `state`, `responded_at` and
`acked_by` still track the transitions of §15.2.)

**New `Incident` fields**, appended after `alerts` in this order. All have defaults, so
v1 positional construction `Incident(id, opened_at, acked_at, resolved_at, alerts)`
still works:

| field | type | default | defined in |
|---|---|---|---|
| `team` | `str \| None` | `None` | §11.7 |
| `policy` | `str \| None` | `None` | §11.7 |
| `responder` | `str \| None` | `None` | §13.3 |
| `state` | `str` | `"triggered"` | §15.1 |
| `acked_by` | `str \| None` | `None` | §15.2 |
| `out_of_band_ack` | `bool` | `False` | §12.8, §15.2 |
| `mitigated_at` | `int \| None` | `None` | §15.2 |
| `responded_at` | `int \| None` | `None` | §15.2 |
| `severity_override` | `Severity \| None` | `None` | §15.2 |
| `merged_into` | `str \| None` | `None` | §15.2 |
| `reopen_count` | `int` | `0` | §15.2 |
| `notifications` | `list[Notification]` | a new empty list per instance | §13.4 |

**Changed properties:**

- `severity` → `severity_override` when it is not `None`, else the max severity of the
  alerts.
- `title` → the message of the first alert whose severity equals the max severity **of
  the alerts**. `severity_override` never affects `title`. (An incident with alerts
  `CRITICAL "db down"` then `LOW "slow"` and override `HIGH` has severity `HIGH` and
  title `"db down"`.)
- `is_open` → `resolved_at is None and merged_into is None`.
- `services`, `fingerprints` are unchanged (computed from `alerts`, which a merge
  extends).

**Changed signature:**

```
correlate(timeline, *, window=300_000, rules=None, schedules=None, policies=None,
          reopen_window=None, audit=None) -> list[Incident]
```

The complete v2 algorithm is §15.5; it replaces the bullet list above. Its differences
from v1, each normative where it is defined:

- "opened earliest" means **lowest `id`**, "most recently opened" means **highest `id`**.
  In v1 these equal `opened_at` order; in v2 a merge can lower a target's `opened_at`
  (§15.2) and the id order still decides.
- An alert joins only an incident of the same team (§11.7).
- An alert may reopen a recently resolved incident (§15.5 step A5) instead of opening a
  new one.
- Note events carrying commands change severity, mitigate and merge (§15.3, §15.5 D).
- Acks record `acked_by` and may be out-of-band (§12.8).
- Each incident gets a `responder` (§13.3) and its escalation `notifications` (§13.4).
- Merged incidents stay in the returned list (with `merged_into` set), in id order.

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

(v2) Changes:

- Every function in this section skips incidents whose `merged_into` is not `None`
  (§15.2). They are not counted, and their durations are not used.
- `mttr`, `summary` and `by_service` use the final `resolved_at`. After a reopen (§15.2)
  that is the last resolve, and a reopened incident that was not resolved again counts
  as open.
- `availability` uses the `severity` property, so a `/sev` override (§15.3) can add an
  incident to, or remove it from, the computation. `by_severity` in `summary` also uses
  the `severity` property.
- `mtta` is still `acked_at - opened_at`; `acked_at` is the first ack (§15.2).
- `summary` gains two keys, appended after `p90_ttr_ms` in this order:
  `"escalated": int` and `"out_of_band_acks": int`, defined in §13.6. **This changes the
  v1 output of `summary` and of the `stats` command (§10):** for one hand-built
  incident, v1 gives a dict with 7 keys ending in `"p90_ttr_ms"`; v2 gives 9 keys ending
  `..., "p90_ttr_ms": ..., "escalated": 0, "out_of_band_acks": 0`.

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

(v2) No grammar change. The ack attr `by` (§4.1) is the field `attr.by`; synthetic SLO
events (§14.6) have `source` `"slo"`, so `source = slo` selects them; note events are
`kind = note`.

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

(v2) Changes:

- Signature: `render(incidents, timeline, *, title="Incident report",
  lookback=3_600_000, audit=None)`.
- Incidents with `merged_into` set are omitted from the Incidents section. The
  `Incidents:` line and the Summary table come from `summary` (§7 v2), which skips them
  too. If every incident is merged, the `No incidents.` form is used.
- The `Severity:` line uses the `severity` property (override included); the heading
  uses `title` (alert maximum only, §6 v2).
- When `audit` is not `None`, each incident block gains the lines of §15.6 after
  `- Suspect changes:`. When `audit` is `None` the output is exactly the v1 output.
- The Window line uses every event of `timeline`, including synthetic SLO events when
  the timeline was fed (§14.6).

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

(v2) Options and positionals may be given in any order (standard `argparse`). Every TS
argument below is parsed with `parse_ts` (no default year); a `TimeParseError` is exit
`2`. Rules files and config files are read as UTF-8; an unreadable rules or config file,
a `RoutingError`, `ScheduleError`, `PolicyError` or `SloError` is exit `2` with a
one-line stderr message and nothing on stdout. For a `RoutingError` the message is
`{rules_path}:{line}: {reason}`.

### 10.1 (v2) New options on `report` and `stats`

```
python -m incidentkit report [v1 options] [--rules RULES] [--config CONFIG] [--reopen DUR] [--slo] [--timeline] FILE...
python -m incidentkit stats  [v1 options] [--rules RULES] [--config CONFIG] [--reopen DUR] [--slo] FILE...
```

Both commands first read and load the rules file and the config (schedules,
policies, and SLOs if `--slo`), so their errors (exit `2`) come before any FILE is
read. Then they run this pipeline, in this order:

1. Load and merge the FILEs into one `Timeline` (v1; parse errors printed as in v1).
2. If `--dedupe DUR`: `timeline = timeline.dedupe(DUR)` (v1).
3. If `--slo`: `--config` is required (without it: usage error, exit `2`). If the
   timeline is not empty, `timeline = feed(timeline, load_slos(config), start, end)`
   (§14.6) with `start` = the first event's `ts` and `end` = the last event's `ts + 1`,
   both taken from the timeline of step 2.
4. If both `--rules` and `--config` are given, compute
   `check_references(rules, policies)` (§13.5); if it is not empty, print one stderr line
   `{rules_path}:{line}: unknown policy {name}` per entry, in order, and exit `2` with
   nothing on stdout.
5. `incidents = correlate(timeline, window=WINDOW, rules=R, schedules=S, policies=P,
   reopen_window=RW, audit=log)` where `R = load_rules(RULES)` or `None`;
   `S = load_schedules(text)` and `P = load_policies(text, S)` from the config text, or
   `None` without `--config`; `RW = parse_duration(DUR)` for `--reopen DUR` or `None`;
   `log = AuditLog()`. A negative `--reopen` is exit `2`.
6. `report` prints `render(incidents, timeline, title=T, audit=log if --timeline else
   None)`; `stats` prints the `summary` JSON as in v1.

Without any v2 option, the output equals the v1 output except for the two new `summary`
keys (§7 v2) in `stats` and the effects of note events, which v2 parses (§4.1 v2) and
correlation applies (§15.3).

### 10.2 (v2) `route`

```
python -m incidentkit route [--year Y] [--tz TZ] [--strict] RULES FILE...
```

Loads RULES with `load_rules`, loads and merges the FILEs (no dedupe), and prints one
line per `alert` event, in timeline order:

```
{format_ts(ts)} {service} {severity name lowercase} -> {route}, {route}, ...
```

listing every element of `routes_for(rules, event)` (§11.5) in order, each written
`{team}/{policy or -} ({rule or default})`. Other event kinds print nothing. Example,
with the rules of §11.6:

```
2024-03-12T08:00:00.000Z search high -> compliance/- (audit-eu), dba/dba-page (db)
```

### 10.3 (v2) `oncall`

```
python -m incidentkit oncall [--schedule NAME] (--at TS | --from TS --to TS) CONFIG
```

Loads the schedules of CONFIG. Exactly one of these forms is required: `--at TS` alone,
or both `--from` and `--to` (any other combination: exit `2`). `--to <= --from`: exit
`2`. `--schedule NAME` selects one schedule; an unknown name is exit `2`. Without it, all
schedules are printed in ascending order of name.

- `--at TS`: one line per schedule, `{name}\t{who_is_on_call(schedule, ts) or -}`.
- `--from A --to B`: for each schedule, one line per segment of
  `final_schedule(schedule, A, B)` (§12.6):
  `{name}\t{format_ts(start)}\t{format_ts(end)}\t{user}`. A schedule without segments
  prints nothing.

### 10.4 (v2) `escalate`

```
python -m incidentkit escalate [--year Y] [--tz TZ] [--strict] [--window DUR] [--dedupe DUR] [--reopen DUR] [--slo] --rules RULES --config CONFIG FILE...
```

`--rules` and `--config` are required (missing: exit `2`). Runs steps 1–5 of §10.1,
then prints, for each incident with `merged_into` equal to `None` in id order, one line
per element of `incident.notifications`:

```
{id}\t{format_ts(ts)}\tL{level}\tP{attempt}\t{user}
```

Nothing else is printed (an incident without notifications prints nothing).

### 10.5 (v2) `slo`

```
python -m incidentkit slo [--year Y] [--tz TZ] [--strict] --config CONFIG --at TS [--from TS] FILE...
```

Loads the SLOs of CONFIG and merges the FILEs (no dedupe, no feed). For each SLO in
config order, with `st = status(slo, timeline, AT)` (§14.4), prints:

```
{name}\t{service}\tbad={st.bad}/{st.total}\tbudget={format_percent(st.remaining)}%
```

If `--from F` is given (`F >= AT`: exit `2`), it then prints every event of
`burn_alerts(slo, timeline, F, AT)` for all SLOs, ordered as a `Timeline` of those
events built by adding each SLO's list in config order (so by `Event.sort_key()`, ties
in config order then rule order), one line each:

```
{format_ts(ts)} {kind} {fingerprint} {message}
```

A config without SLOs prints nothing and exits `0`. `--strict` behaves as in v1 for
`route`, `escalate` and `slo`, the new commands that load FILEs.

## 11. (v2) Routing rules (`incidentkit/routing.py`)

A rules file maps an event to a **team** and an optional escalation **policy** (§13).

### 11.1 Glob matching

`glob_match(pattern, text) -> bool`. The pattern must match the whole of `text`. `*`
matches any run of characters, including the empty run; `?` matches exactly one
character; every other character (including `[`, `]` and `\`) matches only itself,
case-sensitively. `fnmatch` is not equivalent: `fnmatch.fnmatchcase("a[1]", "a[1]")` is
`False`, but `glob_match("a[1]", "a[1]")` is `True`.

Examples: `glob_match("payments-*", "payments-api")` → `True`;
`glob_match("payments-*", "payments")` → `False`; `glob_match("db?", "db1")` → `True`;
`glob_match("db?", "db")` → `False`; `glob_match("*", "")` → `True`;
`glob_match("API", "api")` → `False`.

### 11.2 File grammar

The text is split with `str.splitlines()`; line numbers are 1-based. Each line is
stripped of leading and trailing whitespace (`str.strip()`) and then classified:

- empty → ignored;
- first character `#` → a comment, ignored. A `#` anywhere else is ordinary text
  (`message /a#b/` is a regex containing `#`);
- otherwise a **statement**. Its **keyword** is the text before the first whitespace
  character (the whole line if there is none). Keywords are case-sensitive.

Indentation carries no meaning. A **block** runs from a `route` statement to the next
`end` statement. Statements are allowed as follows:

| keyword | where | arguments |
|---|---|---|
| `route` | top level | exactly one NAME — opens a block named NAME |
| `default` | top level | TEAM, optionally followed by POLICY (1 or 2 arguments) |
| `service` | block | one or more GLOBs |
| `source` | block | one or more GLOBs |
| `severity` | block | exactly one RANGE |
| `attr` | block | `KEY OP VALUE`, see §11.3 |
| `message` | block | `/REGEX/FLAGS`, see §11.3 |
| `team` | block | exactly one NAME |
| `policy` | block | exactly one NAME |
| `continue` | block | none |
| `end` | block | none — closes the block |

Except for `attr` and `message`, the arguments are `stmt.split()[1:]` (runs of
whitespace separate them). NAME, TEAM, POLICY and KEY must fully match
`[A-Za-z0-9_.-]+`. A GLOB is any argument token.

### 11.3 Matchers

`service`, `source`, `severity`, `attr` and `message` are **matchers**. A rule matches an
event iff **every** matcher line of its block matches (a keyword may repeat; repeated
lines are combined with AND too). A block with no matchers matches every event.

- `service G1 G2 …` — matches iff `glob_match(Gi, event.service)` for at least one `Gi`.
- `source G1 G2 …` — the same on `event.source`.
- `severity RANGE` — if RANGE contains no `..`, it is one word `A` read by
  `Severity.parse`, meaning `A..A`. Otherwise `RANGE.split("..")` must give exactly two
  parts `A` and `B`, not both empty; an empty `A` means `INFO`, an empty `B` means
  `CRITICAL`, and a non-empty part is read by `Severity.parse`. So the forms are `A`,
  `A..B`, `A..` and `..B`. Matches iff `lo <= event.severity <= hi`. Errors: `..` alone,
  three or more parts (`high..crit..`), a part `Severity.parse` rejects, and `lo > hi`.
- `attr KEY OP VALUE` — the statement is split with `stmt.split(None, 3)` and must give
  exactly 4 parts: `attr`, KEY, OP, VALUE. OP is `=` or `!=`. VALUE is the fourth part
  verbatim (internal whitespace kept). `=` matches iff `event.attr(KEY) == VALUE`. `!=`
  matches iff `event.attr(KEY) != VALUE`, **so an event without the attribute matches
  `!=`** — unlike §8, where every comparison with a missing field is false.
- `message ARG` — `ARG = stmt[len("message"):].strip()`. It must start with `/` and
  contain a later `/`: with `j = ARG.rfind("/")` and `j > 0`, the regex is `ARG[1:j]` and
  the flags are `ARG[j+1:]`, which must be `""` or `"i"` (`re.IGNORECASE`). Since the
  last `/` ends the regex, `/` inside needs no escape: `message /a/b/` has regex `a/b`.
  The empty regex `//` is allowed. Matches iff `re.search(regex, event.message)` finds a
  match. A regex that `re.compile` rejects is an error.

`team NAME` gives the route's team (required, exactly once per block). `policy NAME`
gives its policy (at most once; absent → `None`). `continue` (at most once) marks the
rule as **non-final** (§11.5).

### 11.4 Validation errors

`parse_rules` scans the lines from first to last and raises `RoutingError(line, reason)`
for the **first** error it reaches; `reason` is free text. The errors and their
reported lines:

1. A keyword not allowed where it appears: unknown keywords; a block keyword at top
   level; `route` or `default` inside a block; `end` at top level → that line.
2. Wrong argument count; invalid NAME, TEAM, POLICY or KEY → that line.
3. Invalid RANGE; OP other than `=`/`!=`; malformed `message` argument, flags other than
   `""`/`"i"`, or a regex `re.compile` rejects → that line.
4. A second `team`, `policy` or `continue` in one block → the line of the second.
5. A `route` NAME equal to an earlier block's NAME → the line of the second `route`.
6. A second `default` statement → its line.
7. A block without `team` → the line of its `route` statement. This check runs when the
   block's `end` is read, so an error on an earlier line inside the block is raised
   first.
8. End of text inside a block → the line of that block's `route` statement.

Examples: in `route a` / `  sevrity high` / `  team x` / `end` (lines 1–4), the error is
at line 2 (unknown keyword). In `route a` / `  severity high` / `end`, the error is at
line 1 (no team), found when line 3 is read. In `route a` / `team x` / `end` /
`route a` / `team y` / `end`, the error is at line 4.

### 11.5 Types and functions

- `Route` — frozen dataclass `(team: str, policy: str | None, rule: str | None)`; `rule`
  is the block NAME, or `None` for the default route.
- `Rule` — frozen dataclass with at least the fields `name: str`, `line: int` (the line
  of its `route` statement), `team: str`, `policy: str | None`, `cont: bool` (true iff
  the block has `continue`), and a method `matches(event) -> bool` (§11.3). Matcher
  storage is implementation-defined.
- `RuleSet` — dataclass with `rules: tuple[Rule, ...]` (file order), `default: Route` and
  `default_line: int | None` (the line of the `default` statement). Without a `default`
  statement, `default == Route("unrouted", None, None)` and `default_line is None`. The
  `default` statement may appear anywhere at top level; its position does not affect
  rule order.
- `parse_rules(text) -> RuleSet` and `load_rules(path) -> RuleSet` (reads UTF-8; an
  `OSError` propagates).
- `routes_for(ruleset, event) -> list[Route]` — walk the rules in order. For each rule
  that matches, append `Route(rule.team, rule.policy, rule.name)`; if that rule is not
  `cont`, return the list immediately. If the walk ends without such a return, append
  `ruleset.default` and return. The list is never empty, and contains the default iff no
  matching rule is final.
- `route(ruleset, event) -> Route` — `routes_for(ruleset, event)[0]`. This **primary
  route** is the one used by correlation (§11.7), escalation (§13) and every CLI command
  except `route` (§10.2).

Routing reads any event kind; correlation routes only alerts.

### 11.6 Example

Rules file (line numbers on the left are not part of the file):

```
 1  # Routing for the shop
 2  route pay-crit
 3    service payments payments-*
 4    severity high..
 5    team payments
 6    policy pay-page
 7  end
 8
 9  route pay-all
10    service payments*
11    team payments
12    policy pay-low
13  end
14
15  route audit-eu
16    attr region = eu
17    team compliance
18    continue
19  end
20
21  route db
22    message /deadlock|timeout/i
23    team dba
24    policy dba-page
25  end
26
27  default ops ops-page
```

`default_line == 27`. Results (all events kind `alert`; attrs listed where present):

| service | severity | attrs | message | `routes_for` (as `team/policy (rule)`) |
|---|---|---|---|---|
| `payments-api` | critical | | `5xx` | `payments/pay-page (pay-crit)` |
| `payments` | medium | | `slow` | `payments/pay-low (pay-all)` |
| `payments-api` | low | | `slow` | `payments/pay-low (pay-all)` |
| `search` | high | region=eu | `Deadlock detected` | `compliance/- (audit-eu)`, `dba/dba-page (db)` |
| `search` | low | region=us | `slow` | `ops/ops-page (default)` |
| `search` | low | region=eu | `slow` | `compliance/- (audit-eu)`, `ops/ops-page (default)` |
| `search` | low | | `Lock TIMEOUT` | `dba/dba-page (db)` |

`route()` is the first element of each list, e.g. `Route("compliance", None,
"audit-eu")` for the fourth row.

### 11.7 Teams in correlation (changes §6)

When `correlate` is given `rules` (§15.5), it routes every alert with
`route(rules, alert)`:

- A new incident gets `team` and `policy` from the primary route of its **first**
  alert. They never change afterwards — not by later alerts and not by merges (§15.2).
- Joining by fingerprint (§6 step 1), by service (§6 step 2) and reopening (§15.5 step
  A5) consider only incidents whose `team` equals the alert's route team. An alert whose
  fingerprint is held by an incident of another team does not join it; it falls through
  to the later steps and may open a new incident with the same fingerprint.
- Ack, resolve and note targeting ignore teams (§15.5 B–D are the v1 rules). An ack
  carrying a fingerprint held by incidents of two teams targets the lowest id.
- With `rules=None`, every alert's team and every incident's team is `None`, so the
  team condition always holds and v1 behaviour is kept.

**Example that differs from v1.** Window 5m, the rules of §11.6, two alerts:
`ts 0, search, HIGH, region=eu, "Deadlock detected"` and `ts 60000, search, LOW,
region=us, "slow"`. v1 (or `rules=None`): one incident `INC-0001` with 2 alerts. v2 with
the rules: `INC-0001` (team `compliance`, policy `None`, 1 alert) and `INC-0002` (team
`ops`, policy `ops-page`, 1 alert).

Synthetic SLO alerts (§14.6) are routed like any alert; `source slo` selects them.

## 12. (v2) On-call schedules (`incidentkit/oncall.py`)

### 12.1 Schedule config

`load_schedules(text) -> dict[str, Schedule]` reads the `"schedules"` key of the config
file (§0), a list of schedule objects; the dict keys are schedule names in file order.
Any violation below raises `ScheduleError`. Integers must be JSON integers (booleans
are rejected).

Schedule object:

| key | required | value |
|---|---|---|
| `name` | yes | non-empty string, unique among schedules |
| `tz` | no | `"Z"`, `"z"` or `"±HH:MM"` (the zone form of §2); default `"+00:00"` |
| `layers` | yes | list (possibly empty) of layer objects, **lowest first** |
| `overrides` | no | list of override objects; default `[]` |

Layer object:

| key | required | value |
|---|---|---|
| `users` | yes | non-empty list of non-empty strings (duplicates allowed) |
| `rotation_days` | yes | integer ≥ 1 |
| `anchor` | yes | `"YYYY-MM-DD"`, a valid date |
| `handoff_time` | no | `"HH:MM"`; default `"00:00"` |
| `handoff_day` | no | one of `"mon"`, `"tue"`, `"wed"`, `"thu"`, `"fri"`, `"sat"`, `"sun"` (exactly, lowercase); when present, `rotation_days` must be a multiple of 7 |
| `restrictions` | no | list of window objects; default `[]` |
| `name` | no | string, informational only |

Window object: `days` (non-empty list of day names as for `handoff_day`), `start`
(`"HH:MM"`), `end` (`"HH:MM"` or `"24:00"`); `start == end` is an error.

Override object: `user` (non-empty string), `start` and `end` (strings read with
`parse_ts(s, default_tz=<schedule tz>)`, no default year; a `TimeParseError` becomes a
`ScheduleError`); `end <= start` is an error. An override covers `[start, end)`.

`"HH:MM"` is exactly two digits, `:`, two digits, with hour `00`–`23` and minute
`00`–`59`; `"24:00"` is accepted only as a window `end`. A time of day is held as ms
since local midnight (`"09:00"` → 32 400 000; `"24:00"` → 86 400 000).

`Schedule` is a dataclass with at least `name: str`, `tz_offset_ms: int` (`"+02:00"` →
7 200 000, `"-05:30"` → −19 800 000, `"Z"` → 0), `layers: tuple` and `overrides: tuple`
in file order. The layer, window and override types are implementation-defined.

### 12.2 Local time

There is no time-zone database: a schedule's zone is a fixed offset. For an instant
`ts` (UTC ms) and a schedule:

- `local = ts + tz_offset_ms`
- `weekday = (local // 86_400_000 + 3) % 7` — `0` is Monday, `6` Sunday (1970-01-01
  was a Thursday)
- `tod = local % 86_400_000` — time of day in ms

`//` and `%` are Python floor operations, so instants before 1970 work.

### 12.3 Rotation

Each layer has an **epoch** `E` (UTC ms) and a **period** `P = rotation_days ×
86_400_000`:

1. `C` = the anchor date's local midnight as a local ms value (days since 1970-01-01 ×
   86 400 000) plus `handoff_time`.
2. If `handoff_day` is set, add `86_400_000 × d` to `C`, where `d` is the smallest of
   `0 … 6` that makes the local weekday of `C` (formula of §12.2 applied to the local
   value) equal to `handoff_day`.
3. `E = C − tz_offset_ms`.

The layer's **rotation user** at `ts` is `users[k % len(users)]` with
`k = (ts − E) // P`. The rotation runs backwards before `E` (negative `k`), and
restrictions never pause it.

### 12.4 Restrictions

A layer is **active** at `ts` iff it has no restrictions, or at least one of its windows
is active at `ts`. With `weekday` and `tod` from §12.2, a window `(days, start, end)` is
active iff:

- `start < end`: `weekday` is in `days` and `start <= tod < end`;
- `start > end` (wraps past midnight): (`weekday` is in `days` and `tod >= start`) or
  (`(weekday − 1) % 7` is in `days` and `tod < end`).

So `{"days": ["fri"], "start": "22:00", "end": "06:00"}` is active from Friday 22:00 to
Saturday 06:00 local, and not on Friday 00:00–06:00.

### 12.5 `who_is_on_call(schedule, ts) -> str | None`

1. If one or more overrides cover `ts` (`start <= ts < end`), the user of the **last**
   such override in list order.
2. Otherwise, the rotation user of the **highest** active layer (the last in the list
   that is active at `ts`).
3. Otherwise `None`.

### 12.6 `final_schedule(schedule, start, end) -> list[tuple[int, int, str]]`

`end <= start` → `ValueError`. Returns the maximal intervals `[s, e)` inside
`[start, end)` on which `who_is_on_call` is constant and not `None`, as
`(s, e, user)` tuples in ascending order. "Maximal" means adjacent intervals with the
same user are merged: no two consecutive tuples satisfy `a.e == b.s and a.user ==
b.user`. Instants where nobody is on call belong to no tuple, so tuples can have gaps
between them. (Non-normative: `who_is_on_call` can only change at rotation handoffs
`E + kP`, at window edges, at local midnights and at override edges; evaluating it at
each such boundary inside the range suffices.)

### 12.7 Example

```json
{"schedules": [
  {"name": "payments", "tz": "+02:00",
   "layers": [
     {"name": "weekly", "users": ["ann", "bob", "cy"], "rotation_days": 7,
      "anchor": "2024-03-01", "handoff_day": "mon", "handoff_time": "09:00"},
     {"name": "business-hours", "users": ["dan"], "rotation_days": 1,
      "anchor": "2024-03-04",
      "restrictions": [{"days": ["mon", "tue", "wed", "thu", "fri"],
                        "start": "09:00", "end": "17:00"}]}
   ],
   "overrides": [{"user": "eve", "start": "2024-03-12T12:00:00",
                  "end": "2024-03-12T13:00:00"}]}
]}
```

The anchor 2024-03-01 is a Friday, so the `weekly` epoch moves to Monday 2024-03-04
09:00 local, `E = 1709535600000` (2024-03-04T07:00:00Z), `P = 604 800 000`. The
override covers `[1710237600000, 1710241200000)` (10:00Z–11:00Z, its strings being local
+02:00 times).

| ts (UTC) | ms | local | answer | why |
|---|---|---|---|---|
| 2024-03-01T00:00Z | 1709251200000 | Fri 02:00 | `cy` | `business-hours` inactive; `k = −1`, `users[2]` |
| 2024-03-11T06:59Z | 1710140340000 | Mon 08:59 | `ann` | `k = 0` (one minute before handoff) |
| 2024-03-12T08:00Z | 1710230400000 | Tue 10:00 | `dan` | `business-hours` active |
| 2024-03-12T10:30Z | 1710239400000 | Tue 12:30 | `eve` | override |
| 2024-03-16T12:00Z | 1710590400000 | Sat 14:00 | `bob` | `k = 1` |

`final_schedule(payments, 1710201600000, 1710288000000)` (2024-03-12T00:00Z to
2024-03-13T00:00Z) is:

```
[(1710201600000, 1710226800000, "bob"),
 (1710226800000, 1710237600000, "dan"),
 (1710237600000, 1710241200000, "eve"),
 (1710241200000, 1710255600000, "dan"),
 (1710255600000, 1710288000000, "bob")]
```

### 12.8 Responders and acks in correlation (changes §4.1 and §6)

- An ack event's `by` attr (§4.1) is the user who acknowledged.
- A user is **on call for an incident at `ts`** iff they are in
  `on_call_users(policies[incident.policy], schedules, ts)` (§13.3). This uses
  `who_is_on_call`, the final layered answer: a user whose lower layer is shadowed by a
  higher layer or by an override at `ts` is **not** on call. (In §12.7, `bob` is the
  `weekly` user at 2024-03-12T08:30Z but `dan` is on call.)
- When an ack sets `acked_at` (§15.2, `apply_ack`), it also sets `acked_by` to its `by`
  (possibly `None`), and sets `out_of_band_ack = True` iff `by` is not `None`, the
  incident's `policy` is a key of `policies`, and `by` is not on call for the incident at
  the ack's `ts`. The ack is applied in every case; out-of-band only marks it.
- An ack that does not set `acked_at` (an ack after a reopen, §15.2) changes neither
  `acked_by` nor `out_of_band_ack`. An ignored ack (§15.5 B) changes nothing.
- An incident's `responder` is defined in §13.3. `summary` counts out-of-band acks
  (§13.6).

## 13. (v2) Escalation (`incidentkit/escalation.py`)

### 13.1 Policy config

`load_policies(text, schedules) -> dict[str, Policy]` reads the `"policies"` key of the
config file (§0), a list of policy objects; `schedules` is the dict from
`load_schedules` (§12.1). Dict keys are policy names in file order. Any violation raises
`PolicyError`. Integers must be JSON integers (booleans rejected).

| key | required | value |
|---|---|---|
| `name` | yes | non-empty string, unique among policies |
| `levels` | yes | non-empty list of level objects, level 1 first |
| `repeat` | no | integer ≥ 0; default `0` |

Level object: exactly one of `schedule` (a string that is a key of `schedules`) and
`users` (non-empty list of non-empty strings); and `delay_minutes` (required, integer
≥ 0). Both or neither of `schedule`/`users`, or an unknown schedule name, is an error.

Types (frozen dataclasses): `Level(delay_minutes: int, schedule: str | None, users:
tuple[str, ...])` — `users == ()` for a schedule level, `schedule is None` for a users
level; `Policy(name: str, levels: tuple[Level, ...], repeat: int)`;
`Notification(ts: int, level: int, user: str, attempt: int)` — `level` and `attempt`
are 1-based.

Throughout §13, a schedule name that is not a key of the `schedules` argument has nobody
on call (this arises only with hand-built policies).

### 13.2 `simulate(policy, schedules, start, stop=None) -> list[Notification]`

The policy runs `repeat + 1` **passes** (attempts). The steps are the pairs
`(attempt, level)` in the order `(1, 1), (1, 2), …, (1, n), (2, 1), …, (repeat+1, n)`.
Step times: the first step is at `start`; each later step is at the previous step's time
plus the **previous** step's level `delay_minutes × 60_000`. So a level's delay is how
long the policy waits on it before moving on, and the last level's delay in a pass
delays the next pass; the last step's delay is unused.

At each step with time `T`, if `stop` is not `None` and `T >= stop`, simulation ends
(later steps are never earlier). Otherwise:

- schedule level: `u = who_is_on_call(schedules[name], T)`; if `u` is not `None`, emit
  `Notification(T, level, u, attempt)`; if it is `None`, emit nothing, and timing
  continues;
- users level: emit one notification per user, in list order, all at `T`.

The result is in emission order (non-decreasing `ts`).

Example: `ops-page` of §13.7 (one users level `["olga", "omar"]`, delay 5, repeat 0)
from `start = 0`: `[Notification(0, 1, "olga", 1), Notification(0, 1, "omar", 1)]`.

### 13.3 On-call users and responder

- `on_call_users(policy, schedules, ts) -> frozenset[str]` — the union over **all**
  levels (whether or not escalation would reach them): for a schedule level,
  `who_is_on_call(schedule, ts)` when not `None`; for a users level, all its users.
- `responder(policy, schedules, ts) -> str | None` — from level 1 only: a schedule
  level gives `who_is_on_call(schedule, ts)`; a users level gives its first user.

(Changes §6.) `correlate` (§15.5 A6) sets a new incident's `responder` to
`responder(policies[policy], schedules, opened_at)` when its `policy` is a key of
`policies`, else `None`. It is set once, at creation, and is not recomputed on merge or
reopen. `on_call_users` decides out-of-band acks (§12.8).

### 13.4 `escalate(incident, policies, schedules) -> list[Notification]`

Returns `[]` if `incident.merged_into` is not `None` or `incident.policy` is not a key
of `policies`. Otherwise returns `simulate(policies[incident.policy], schedules,
incident.opened_at, stop)` where `stop` is:

1. `incident.responded_at` if not `None` (the first ack, mitigate or resolve, §15.2);
2. else the minimum of the non-`None` values among `acked_at`, `mitigated_at`,
   `resolved_at` (for hand-built incidents);
3. else `None`.

(Changes §6.) After its walk, `correlate` sets `incident.notifications =
escalate(incident, policies, schedules)` for every incident (§15.5 E). Consequences:
a merge that lowered a target's `opened_at` (§15.2) moves its escalation start; a
mitigation stops escalation even without an ack; a reopen does not restart escalation,
because `responded_at` keeps the first response. A notification at exactly `stop` is
not sent.

### 13.5 `check_references(ruleset, policies) -> list[tuple[int, str]]`

Unknown policy names in a rule set (§11.5): one `(rule.line, rule.policy)` for each rule,
in file order, whose `policy` is not `None` and not a key of `policies`; then, if
`ruleset.default.policy` is not `None` and not a key, `(ruleset.default_line,
ruleset.default.policy)`. The list is then stably sorted by line. The CLI rejects
non-empty results (§10.1 step 4); `correlate` itself tolerates unknown policies (they
give no responder and no notifications).

### 13.6 Metrics (changes §7)

- `mtta` stays `acked_at − opened_at` (unaffected by escalation).
- `summary["escalated"]` — the number of non-merged incidents whose `notifications`
  contain at least one notification with `level >= 2`. A pass-2 notification of level 1
  does not count; a level 2 that was reached but had nobody on call does not count.
- `summary["out_of_band_acks"]` — the number of non-merged incidents with
  `out_of_band_ack` true (§12.8).

### 13.7 Example

Config `"policies"` (with the schedules of §12.7):

```json
"policies": [
  {"name": "pay-page", "repeat": 1, "levels": [
     {"schedule": "payments", "delay_minutes": 10},
     {"users": ["lead"], "delay_minutes": 15}]},
  {"name": "ops-page", "levels": [{"users": ["olga", "omar"], "delay_minutes": 5}]}
]
```

`check_references` of the §11.6 rules against these policies is
`[(9, "pay-low"), (21, "dba-page")]`.

`simulate(pay-page, schedules, 1710230400000)` (2024-03-12T08:00Z, no stop) gives steps
at 08:00 (1,1), 08:10 (1,2), 08:25 (2,1), 08:35 (2,2):

```
[Notification(1710230400000, 1, "dan",  1),
 Notification(1710231000000, 2, "lead", 1),
 Notification(1710231900000, 1, "dan",  2),
 Notification(1710232500000, 2, "lead", 2)]
```

With `stop = 1710232200000` (08:30Z) the first three are returned.
`on_call_users(pay-page, schedules, 1710232200000) == frozenset({"dan", "lead"})`, and
`responder(pay-page, schedules, 1710230400000) == "dan"`.

## 14. (v2) SLOs and burn-rate alerts (`incidentkit/slo.py`)

All SLO arithmetic is exact, with `fractions.Fraction`; floats never appear. Rounding
happens only when formatting (§14.5).

### 14.1 SLO config

`load_slos(text) -> list[Slo]` reads the `"slos"` key of the config file (§0), in file
order. Any violation raises `SloError`. Integers must be JSON integers (booleans
rejected).

| key | required | value |
|---|---|---|
| `name` | yes | string fully matching `[A-Za-z0-9_.-]+`, unique among SLOs |
| `service` | yes | non-empty string |
| `objective` | yes | a **string** fully matching `[0-9]+(\.[0-9]+)?` whose value is strictly between 0 and 100 (`"99.9"`, `"99.95"`, `"95"`). A JSON number is an error. |
| `window_days` | yes | integer, 1–365 |
| `bad_severity` | no | string read by `Severity.parse`; default `"high"` |
| `burn_alerts` | no | list of burn-rule objects (possibly empty: no alerts); default below |

Burn-rule object: `long` and `short` (strings read by `parse_duration`; each must be a
positive multiple of 60 000 ms, and `short < long`; a `DurationParseError` becomes an
`SloError`), `factor` (a string fully matching `[0-9]+(\.[0-9]+)?` with value > 0),
`severity` (string for `Severity.parse`; default `"critical"`). When `burn_alerts` is
absent it is:

```json
[{"long": "1h", "short": "5m", "factor": "14.4", "severity": "critical"},
 {"long": "6h", "short": "30m", "factor": "6", "severity": "high"}]
```

Types (frozen dataclasses): `BurnRule(long_ms: int, short_ms: int, factor: Fraction,
severity: Severity)`; `Slo(name: str, service: str, objective: Fraction, window_days:
int, bad_severity: Severity, burn_alerts: tuple[BurnRule, ...])`, where
`objective == Fraction("99.9")` for `"99.9"` and `factor == Fraction("14.4")`.

`budget_fraction(slo) -> Fraction` = `1 − slo.objective / 100` (`"99.9"` → `1/1000`).

### 14.2 Bad minutes

Time is bucketed into **minutes**: the minute index of `ts` is `ts // 60_000` (UTC,
floor). `bad_minutes(slo, timeline) -> set[int]` is the set of minute indexes of the
events `e` with `e.kind == "log"`, `e.service == slo.service` (exact string equality,
no globs) and `e.severity >= slo.bad_severity`. Every other minute is **good**,
including minutes before the first event and minutes without any event. Several bad
logs in one minute make one bad minute. Only `log` events count: alerts, including the
synthetic SLO alerts of §14.6, never do, so feeding burn alerts back into a timeline
(§14.6) cannot change any SLO computation.

### 14.3 Burn rate

`burn_rate(slo, timeline, end_minute, minutes) -> Fraction` =
`Fraction(b, minutes) / budget_fraction(slo)`, where `b` is the number of bad minutes
`m` with `end_minute − minutes <= m < end_minute`. `minutes <= 0` → `ValueError`.
Example: objective `"99.9"`, one bad minute inside a 60-minute window →
`Fraction(1, 60) / Fraction(1, 1000) == Fraction(50, 3)`.

### 14.4 `status(slo, timeline, at) -> SloStatus`

`SloStatus` is a frozen dataclass `(name: str, total: int, bad: int, good: int,
allowed_bad: Fraction, remaining: Fraction)`:

- `end_minute = at // 60_000`; the window is the `total = window_days × 1440` minutes
  `[end_minute − total, end_minute)`. The minute containing `at` is never in the window,
  even when `at` is a multiple of 60 000.
- `bad` = bad minutes in the window; `good = total − bad`;
- `allowed_bad = total × budget_fraction(slo)`;
- `remaining = 1 − Fraction(bad) / allowed_bad` (negative once the budget is spent).

### 14.5 Formatting

- `format_fixed(value, places=2) -> str` — `r = round(value, places)` (Python's
  `round` on a `Fraction`: exact, ties to even). Written as an optional `-` (iff
  `r < 0`), the integer part without leading zeros (at least `0`), `.`, and exactly
  `places` digits. Examples: `Fraction(1, 8)` → `"0.12"`; `Fraction(3, 8)` → `"0.38"`;
  `Fraction(-1, 8)` → `"-0.12"`; `Fraction(-1, 1000)` → `"0.00"`; `Fraction(50, 3)` →
  `"16.67"`; `Fraction(6)` → `"6.00"`.
- `format_percent(value) -> str` — `format_fixed(value × 100)`. `Fraction(211, 216)` →
  `"97.69"`.
- A burn rule's **label** is `f"{format_duration(long_ms)}/{format_duration(short_ms)}"`
  (§2): `"1h/5m"`; a rule written `"long": "60m"` has label `"1h/5m"` too.
- A factor is written `format_fixed(factor, 1)` stripped of a trailing `".0"`:
  `Fraction("14.4")` → `"14.4"`, `Fraction(6)` → `"6"`, `Fraction("2.25")` → `"2.2"`.

### 14.6 Burn alerts

`burn_alerts(slo, timeline, start, end) -> list[Event]`; `end <= start` → `ValueError`.

For a rule `r`, `cond_r(t)` holds iff, with `em = t // 60_000`, both
`burn_rate(slo, timeline, em, r.long_ms // 60_000) >= r.factor` and
`burn_rate(slo, timeline, em, r.short_ms // 60_000) >= r.factor`.

The **evaluation instants** are `t = ceil(start / 60_000) × 60_000` (computed as
`-(-start // 60_000) * 60_000`), then every 60 000 ms while `t < end`. At each instant,
for each rule in order:

- `cond_r(t)` and not `cond_r(t − 60_000)` → an **alert** event;
- not `cond_r(t)` and `cond_r(t − 60_000)` → a **resolve** event.

`cond_r(t − 60_000)` is computed normally even when `t − 60_000 < start`, so a burn
already in progress at the first instant produces no alert. The result is ordered by
instant, then rule order.

Event fields (both kinds): `ts = t`; `service = slo.service`; `severity = r.severity`;
`source = "slo"`; `fingerprint = f"slo:{slo.name}:{label}"`;
`attrs = (("slo", slo.name), ("window", label))`. Messages:

- alert: `f"SLO {slo.name} burning {format_fixed(long_rate)}x over {format_duration(r.long_ms)} (threshold {factor}x)"`,
  where `long_rate` is the long-window burn rate at `t` and `factor` is written as in
  §14.5;
- resolve: `f"SLO {slo.name} recovered over {format_duration(r.long_ms)}"`.

`feed(timeline, slos, start, end) -> Timeline` returns a new `Timeline` holding the
events of `timeline` followed (with `add`, §5) by `burn_alerts(slo, timeline, start,
end)` for each SLO in list order; `timeline` itself is unchanged. All burn alerts are
computed from the original `timeline`.

**Into correlation (changes §6).** Fed burn alerts are ordinary `alert`/`resolve`
events for `correlate`: they are routed (§11.7; `source slo` selects them), join an
incident holding their fingerprint (§6 step 1), or join by service an open incident of
the same team (§6 step 2). Their resolve targets by fingerprint, so a burn resolve
resolves whichever open incident holds that fingerprint, including a non-SLO incident
that the burn alert joined by service.

### 14.7 Example

SLO `{"name": "api-avail", "service": "api", "objective": "99.9", "window_days": 30}`
(default burn rules). Timeline: one `log` event, service `api`, severity `HIGH`, at
2024-03-01T10:00:30Z (`1709287230000`, minute 28488120).

`burn_alerts(slo, timeline, 1709287200000, 1709287800000)` (10:00Z to 10:10Z):
`cond` for `1h/5m` is false at 10:00 (the bad minute is not yet in any window), true
from 10:01 to 10:05, false from 10:06. The long rate at 10:01 is `Fraction(50, 3)`;
the short rate is `200`. `6h/30m` never holds (long rate `Fraction(25, 9)` < 6). Result:

```
Event(1709287260000, "alert", "api", Severity.CRITICAL,
      "SLO api-avail burning 16.67x over 1h (threshold 14.4x)", "slo",
      "slo:api-avail:1h/5m", (("slo", "api-avail"), ("window", "1h/5m")))
Event(1709287560000, "resolve", "api", Severity.CRITICAL,
      "SLO api-avail recovered over 1h", "slo",
      "slo:api-avail:1h/5m", (("slo", "api-avail"), ("window", "1h/5m")))
```

`status(slo, timeline, 1709287800000)`: `total 43200`, `bad 1`, `good 43199`,
`allowed_bad Fraction(216, 5)`, `remaining Fraction(211, 216)`;
`format_percent(remaining) == "97.69"`. With 3 bad minutes, `remaining` would be
`Fraction(67, 72)` → `"93.06"`.

Fed into `correlate` without rules, the alert opens `INC-0001` (service `api`, opened
2024-03-01T10:01:00Z) and the resolve closes it at 10:06:00Z.

## 15. (v2) Incident lifecycle and audit log (`incidentkit/lifecycle.py`)

### 15.1 States

Module constants (strings): `TRIGGERED = "triggered"`, `ACKNOWLEDGED = "acknowledged"`,
`MITIGATED = "mitigated"`, `RESOLVED = "resolved"`, `MERGED = "merged"`, and
`ACTIVE_STATES = frozenset({TRIGGERED, ACKNOWLEDGED, MITIGATED})`. `Incident.state`
(§6 v2) holds one of the five. A new incident is `triggered`. An incident in an active
state is open (`is_open`, §6 v2): it can receive alerts, acks and resolves.

### 15.2 Transitions

Each function below first checks legality. If the transition is illegal it raises
`IllegalTransition(incident.id, incident.state, action)` (with `action` the name in the
table) and changes nothing, logging nothing. If legal, it applies the effects and then,
when `log` is not `None`, appends the audit entries listed (§15.4). Wherever an effect
says "set X to `ts` if `None`", an existing value is kept.

| function | action | legal from | to | effects |
|---|---|---|---|---|
| `apply_open(incident, log=None)` | `open` | any (no check) | unchanged | logs only; the entry's `ts` is `incident.opened_at`, `from_state` is `"none"` |
| `apply_ack(incident, ts, log=None, *, by=None, on_call=None)` | `ack` | `triggered`; or `mitigated` with `acked_at is None` | `acknowledged` from `triggered`; `mitigated` from `mitigated` | see below |
| `apply_mitigate(incident, ts, log=None)` | `mitigate` | `triggered`, `acknowledged` | `mitigated` | `mitigated_at` and `responded_at` set to `ts` if `None` |
| `apply_resolve(incident, ts, log=None)` | `resolve` | any active state | `resolved` | `resolved_at = ts`; `responded_at` set to `ts` if `None` |
| `apply_reopen(incident, ts, log=None, *, reopen_window)` | `reopen` | `resolved` | `triggered` | `resolved_at = None`; `reopen_count += 1` |
| `apply_severity(incident, ts, severity, log=None)` | `severity` | any active state | unchanged | `severity_override = severity` |
| `apply_merge(source, target, ts, log=None)` | `merge` | see below | source → `merged` | see below |

`apply_ack`: if `acked_at is None`, sets `acked_at = ts`, `acked_by = by`, and
`out_of_band_ack = (by is not None and on_call is not None and by not in on_call)`
(`on_call` is a set of users or `None`, §12.8). If `acked_at` is already set (an ack
after a reopen), those three fields are unchanged and only the state changes. In both
cases `responded_at` is set to `ts` if `None`. An ack in `acknowledged`, in `mitigated`
with `acked_at` set, or in `resolved`/`merged` is illegal.

`apply_reopen`: illegal unless the state is `resolved`. If
`ts − resolved_at > reopen_window`, raises `ReopenWindowError(incident.id, "resolved",
"reopen")` instead (nothing changes). `ts − resolved_at == reopen_window` is allowed.
`acked_at`, `acked_by`, `mitigated_at` and `responded_at` are kept, so `mtta` (§7) and
the escalation stop (§13.4) still use the first response, and `mttr` uses the final
resolve.

`apply_severity` may set the same severity the incident already has; it is still a
legal transition and is logged.

`apply_merge(source, target, ts, log)`: legal iff `source.id != target.id`, both states
are active, and `source.team == target.team`; otherwise
`IllegalTransition(source.id, source.state, "merge")`. Effects:

- `target.alerts` = the stable sort by `Event.sort_key()` of `target.alerts +
  source.alerts` (on equal keys the target's alerts come first);
- `target.opened_at = min(target.opened_at, source.opened_at)`;
- if `source.acked_at` is not `None` and (`target.acked_at` is `None` or
  `source.acked_at < target.acked_at`), the target takes the source's `acked_at`,
  `acked_by` and `out_of_band_ack`;
- `target.mitigated_at` and `target.responded_at` become the minimum of the non-`None`
  values of the two incidents (or stay `None`);
- the target's `state`, `team`, `policy`, `responder`, `severity_override`,
  `reopen_count` and `id` are unchanged;
- `source.state = "merged"`, `source.merged_into = target.id`; the source's other
  fields (including `alerts`) are unchanged.

### 15.3 Note commands

`parse_command(text) -> Command | None`. `Command` is a frozen dataclass `(name: str,
incident: str, arg: str | None)`.

1. `s = text.strip()`. If `s` does not start with `/`, return `None` (an ordinary note).
2. Otherwise split `s` with `s.split()`. The first token without its `/`, lowercased,
   is the command name. An incident ID must fully match `INC-[0-9]{4,}` (case-sensitive).
3. Forms:
   - `/sev ID LEVEL` — exactly 3 tokens; `LEVEL` read by `Severity.parse`;
     `Command("sev", ID, <severity name lowercase>)`.
   - `/mitigate ID` — exactly 2 tokens; `Command("mitigate", ID, None)`.
   - `/merge ID1 into ID2` — exactly 4 tokens, the third equal to `into`
     case-insensitively, `ID1 != ID2`; `Command("merge", ID1, ID2)` (ID1 is merged into
     ID2).
4. Anything else starting with `/` raises `CommandError`: an unknown command (`/ack`,
   `/`), a wrong token count, a bad ID, a bad level, a missing `into`, equal IDs.

Examples: `parse_command("  /SEV INC-0002 sev1 ")` → `Command("sev", "INC-0002",
"critical")`; `parse_command("rolled back")` → `None`; `parse_command("/merge INC-0003
into INC-0001")` → `Command("merge", "INC-0003", "INC-0001")`;
`parse_command("/sev inc-0002 high")` → `CommandError`.

**Example that differs from v1.** Alert `ts 0, api, LOW, "disk"` and note
`ts 10000, api, "/sev INC-0001 critical"`. v1 ignores notes: `INC-0001` has severity
`LOW`. v2: severity `CRITICAL` (title still `"disk"`), so `availability` (§7) counts the
incident and `summary["by_severity"]["critical"] == 1`.

### 15.4 Audit log

`AuditEntry` — frozen dataclass `(ts: int, incident: str, action: str, from_state: str,
to_state: str, detail: str = "")` with a method `line() -> str`:

```
{format_ts(ts)} {incident} {action} {from_state}->{to_state}
```

followed by `" " + detail` when `detail` is not empty.

`AuditLog()` — append-only:

- `append(entry)` — raises `ValueError` if the log is not empty and `entry.ts` is less
  than the last entry's `ts`; equal is allowed.
- `entries` — a property returning a `tuple` of all entries in append order.
- `len(log)` and iteration in append order.
- `for_incident(incident_id) -> list[AuditEntry]` — entries with that `incident`, in
  order.
- `render() -> str` — `"".join(e.line() + "\n" for e in entries)`; `""` when empty.

There is no method to remove or change entries.

Entries appended by §15.2 (`S` is the incident's state before, `S'` after):

| action | entry | `detail` |
|---|---|---|
| open | `(opened_at, id, "open", "none", state)` | `""` |
| ack | `(ts, id, "ack", S, S')` | `by={by}` when `by` is not `None`, then ` out-of-band` when this ack set `out_of_band_ack`; else `""` |
| mitigate | `(ts, id, "mitigate", S, "mitigated")` | `""` |
| resolve | `(ts, id, "resolve", S, "resolved")` | `""` |
| reopen | `(ts, id, "reopen", "resolved", "triggered")` | `after={format_duration(ts − old resolved_at)}` |
| severity | `(ts, id, "severity", S, S)` | `{old}->{new}` — lowercase names of the `severity` property before and after |
| merge | first `(ts, source.id, "merge", S_source, "merged")`, then `(ts, target.id, "absorb", S_target, S_target)` | `into={target.id}`; `from={source.id}` |

So an out-of-band ack by `bob` renders
`2024-03-12T08:30:00.000Z INC-0001 ack triggered->acknowledged by=bob out-of-band`.

### 15.5 Correlation with the lifecycle (replaces the §6 algorithm)

```
correlate(timeline, *, window=300_000, rules=None, schedules=None, policies=None,
          reopen_window=None, audit=None) -> list[Incident]
```

`rules` is a `RuleSet` (§11.5) or `None`; `schedules` and `policies` are dicts as
returned by `load_schedules`/`load_policies`, `None` meaning `{}`; `reopen_window` is
`None` (reopening disabled) or an int ms ≥ 0 (negative → `ValueError`); `audit` is an
`AuditLog` or `None`. Every lifecycle call below passes `audit` as `log`, so `audit`
receives entries in walk order (it may already hold entries; §15.4 `append` rules
apply).

Walk the timeline in order. "Open" means `is_open` (§6 v2).

**A. `alert` event `e`.**

1. `r = route(rules, e)` if `rules` is not `None`; `team = r.team`, `policy = r.policy`;
   otherwise `team = policy = None`.
2. **Eligible** incidents: open and `incident.team == team`.
3. If `e.fingerprint` is not `None` and some eligible incident has it among its
   `fingerprints`: append `e` to the lowest-id such incident. Done.
4. Else, among eligible incidents that include `e.service` and whose latest alert has
   `ts >= e.ts − window`, append `e` to the one whose latest alert is most recent (tie:
   lowest id). Done.
5. Else, if `reopen_window` is not `None`: the **candidates** are incidents with state
   `resolved`, `team == team` and `e.ts − resolved_at <= reopen_window`. If
   `e.fingerprint` is not `None` and some candidates have it among their
   `fingerprints`, keep only those; otherwise keep the candidates that include
   `e.service`. If any remain, take the one with the greatest `resolved_at` (tie: lowest
   id), call `apply_reopen(inc, e.ts, audit, reopen_window=reopen_window)`, then append
   `e` to its alerts. Done.
6. Else open a new incident: next id, `opened_at = e.ts`, `alerts = [e]`, `team`,
   `policy`, `responder` as in §13.3; then `apply_open(inc, audit)`.

Appending an alert in steps 3 and 4 changes no state and logs nothing: a new alert does
not move an acknowledged or mitigated incident back to `triggered`.

**B. `ack` event `e`.** Target (v1 rule, ids per §6 v2): if `e.fingerprint` is not
`None`, the lowest-id open incident with that fingerprint (none → no target, no
fallback); otherwise the highest-id open incident that includes `e.service`. No target:
ignore. Else call `apply_ack(target, e.ts, audit, by=e.attr("by"), on_call=oc)` with
`oc = on_call_users(policies[target.policy], schedules, e.ts)` if `target.policy` is a
key of `policies`, else `None`. An `IllegalTransition` is caught and the event ignored.

**C. `resolve` event `e`.** Target as in B; no target: ignore; else
`apply_resolve(target, e.ts, audit)`.

**D. `note` event `e`.** `cmd = parse_command(e.message)`; if it returns `None` or
raises `CommandError`, ignore the event. Find the incident whose `id == cmd.incident`
among those created so far; none → ignore. Then:

- `sev`: `apply_severity(inc, e.ts, Severity.parse(cmd.arg), audit)`;
- `mitigate`: `apply_mitigate(inc, e.ts, audit)`;
- `merge`: find the incident with `id == cmd.arg` (none → ignore);
  `apply_merge(inc, that, e.ts, audit)`.

An `IllegalTransition` is caught and the event ignored. The note's `service` and
`fingerprint` play no part.

Other kinds are ignored.

**E. After the walk**, for every incident in id order:
`incident.notifications = escalate(incident, policies, schedules)` (§13.4).

Return all incidents, merged ones included, in id order.

**Example that differs from v1 (reopen).** Events: alert `ts 0, api`; resolve
`ts 60000, api`; alert `ts 120000, api` (no fingerprints). v1, or `reopen_window=None`:
`INC-0001` resolved at 60000 (`mttr` 60000) and `INC-0002` open. With
`reopen_window=300_000`: only `INC-0001`, open, 2 alerts, `reopen_count == 1`,
`resolved_at is None`, so `mttr` is `None` and `summary["open"] == 1`. With
`reopen_window=30_000` the gap (60 000) is too long and the v1 result is kept.

### 15.6 Report timeline (changes §9)

When `render` is given `audit` (§9 v2), each incident block gets these lines after
`- Suspect changes: …`:

```
- Team: {team or —}
- Policy: {policy or —}
- Responder: {responder or —}
- Acknowledged by: {acked_by or —}
- State: {state}
- Notifications: {len(notifications)}
- Timeline:
  - {entry.line()}
```

`Acknowledged by` gets the suffix ` (out-of-band)` when `out_of_band_ack` is true. The
`- Timeline:` line is followed by one `  - ` line (two spaces, dash, space) per entry of
`audit.for_incident(id)`, in order. If there are none, the line is `- Timeline: none`
and nothing follows it. Entries of a merged source stay under that (omitted) incident;
the target shows only its own entries, including `absorb`.

### 15.7 Worked example

Rules: §11.6. Config: schedules of §12.7 plus policies of §13.7. `reopen_window =
600_000`. Events (JSON Lines, §4.1):

```
{"at": "2024-03-12T08:00:00Z", "svc": "payments-api", "type": "trigger", "sev": "critical", "summary": "Payment API 5xx", "dedup_key": "pay-5xx"}
{"at": "2024-03-12T08:30:00Z", "svc": "payments-api", "type": "ack", "dedup_key": "pay-5xx", "by": "bob"}
{"at": "2024-03-12T08:40:00Z", "svc": "payments-api", "type": "note", "summary": "/sev INC-0001 high"}
{"at": "2024-03-12T08:45:00Z", "svc": "payments-api", "type": "note", "summary": "/mitigate INC-0001"}
{"at": "2024-03-12T09:00:00Z", "svc": "payments-api", "type": "resolve", "dedup_key": "pay-5xx"}
{"at": "2024-03-12T09:03:00Z", "svc": "payments-api", "type": "trigger", "sev": "critical", "summary": "Payment API 5xx again", "dedup_key": "pay-5xx"}
{"at": "2024-03-12T09:20:00Z", "svc": "payments-api", "type": "resolve", "dedup_key": "pay-5xx"}
```

Walk: the first alert routes to `payments/pay-page (pay-crit)` and opens `INC-0001` with
responder `dan` (§13.7). `bob` acks at 08:30; the users on call are `{dan, lead}`, so
the ack is out-of-band (`bob` is only the shadowed `weekly` user, §12.8). The notes set
the severity to `HIGH` and mitigate. The 09:03 alert finds no open incident, routes to
team `payments`, and reopens `INC-0001` (3 minutes after the resolve, within 10
minutes). After the walk, escalation stops at `responded_at` = 08:30, giving the three
notifications of §13.7 (`L2` reached, so the incident counts as escalated).

`audit.render()`:

```
2024-03-12T08:00:00.000Z INC-0001 open none->triggered
2024-03-12T08:30:00.000Z INC-0001 ack triggered->acknowledged by=bob out-of-band
2024-03-12T08:40:00.000Z INC-0001 severity acknowledged->acknowledged critical->high
2024-03-12T08:45:00.000Z INC-0001 mitigate acknowledged->mitigated
2024-03-12T09:00:00.000Z INC-0001 resolve mitigated->resolved
2024-03-12T09:03:00.000Z INC-0001 reopen resolved->triggered after=3m
2024-03-12T09:20:00.000Z INC-0001 resolve triggered->resolved
```

`summary([INC-0001])`:

```
{"count": 1, "open": 0, "resolved": 1,
 "by_severity": {"critical": 0, "high": 1, "medium": 0, "low": 0, "info": 0},
 "mtta_ms": 1800000, "mttr_ms": 4800000, "p90_ttr_ms": 4800000,
 "escalated": 1, "out_of_band_acks": 1}
```

The incident's block in `render(incidents, timeline, audit=audit)` (title from the
CRITICAL alert, severity from the override; no deploys, so no suspects):

```
### INC-0001 — Payment API 5xx
- Severity: high
- Services: payments-api
- Opened: 2024-03-12T08:00:00.000Z
- Acknowledged: 2024-03-12T08:30:00.000Z
- Resolved: 2024-03-12T09:20:00.000Z
- Duration: 1h20m
- Alerts: 2
- Suspect changes: none
- Team: payments
- Policy: pay-page
- Responder: dan
- Acknowledged by: bob (out-of-band)
- State: resolved
- Notifications: 3
- Timeline:
  - 2024-03-12T08:00:00.000Z INC-0001 open none->triggered
  - 2024-03-12T08:30:00.000Z INC-0001 ack triggered->acknowledged by=bob out-of-band
  - 2024-03-12T08:40:00.000Z INC-0001 severity acknowledged->acknowledged critical->high
  - 2024-03-12T08:45:00.000Z INC-0001 mitigate acknowledged->mitigated
  - 2024-03-12T09:00:00.000Z INC-0001 resolve mitigated->resolved
  - 2024-03-12T09:03:00.000Z INC-0001 reopen resolved->triggered after=3m
  - 2024-03-12T09:20:00.000Z INC-0001 resolve triggered->resolved
```

`python -m incidentkit escalate --rules rules.txt --config config.json --reopen 10m
events.jsonl` would exit `2` here, because the §11.6 rules reference `pay-low` and
`dba-page`, which the config lacks (§10.1 step 4); with those two policies added to the
config it prints:

```
INC-0001	2024-03-12T08:00:00.000Z	L1	P1	dan
INC-0001	2024-03-12T08:10:00.000Z	L2	P1	lead
INC-0001	2024-03-12T08:25:00.000Z	L1	P2	dan
```

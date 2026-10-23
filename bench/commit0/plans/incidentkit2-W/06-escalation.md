# 06: Escalation policies

**What to build:** Implement `incidentkit/escalation.py` per SPEC §13 (lines 1100–1227)
and the config-file rules of SPEC §0: `load_policies(text, schedules)` with validation
(`PolicyError`; unique non-empty names, non-empty `levels`, exactly one of
`schedule`/`users`, schedule must be a key of `schedules`, `delay_minutes` integer ≥ 0,
`repeat` integer ≥ 0 default 0, booleans rejected); `simulate(policy, schedules, start,
stop=None)` (steps across `repeat + 1` passes, previous step's delay, stop at `T >= stop`,
schedule levels with nobody on call emit nothing); `on_call_users`; `responder`;
`escalate(incident, policies, schedules)` (stop = `responded_at`, else min of
acked/mitigated/resolved, else None; `[]` when merged or policy unknown);
`check_references(ruleset, policies)` (rules in file order then default, stably sorted by
line). A schedule name absent from `schedules` means nobody on call. Keep the tracer's
`Level`, `Policy`, `Notification` dataclasses.

**Blocked by:** 01

**Status:** ready-for-agent

**Files:** incidentkit/escalation.py

**Context:**
`SPEC.md:1102 §13.1 load_policies(text, schedules) -> dict[str, Policy]`
`SPEC.md:1119 Level(delay_minutes, schedule, users), Policy(name, levels, repeat), Notification(ts, level, user, attempt)`
`SPEC.md:1128 §13.2 simulate(policy, schedules, start, stop=None) -> list[Notification]`
`SPEC.md:1150 §13.3 on_call_users(policy, schedules, ts) -> frozenset[str]; responder(policy, schedules, ts)`
`SPEC.md:1163 §13.4 escalate(incident, policies, schedules) -> list[Notification]`
`SPEC.md:1181 §13.5 check_references(ruleset, policies) -> list[tuple[int, str]]`
`SPEC.md:1199 §13.7 worked example`
`incidentkit/oncall.py  who_is_on_call(schedule, ts) -> str | None  — schedule levels (ticket 05)`
`incidentkit/oncall.py  load_schedules(text)  — needed to build §12.7 schedules in checks`
`incidentkit/routing.py  RuleSet(rules, default, default_line); Rule.line, Rule.policy  — check_references input`
`incidentkit/correlate.py  Incident fields opened_at, acked_at, mitigated_at, resolved_at, responded_at, merged_into, policy`
`incidentkit/errors.py  PolicyError(message)`

- [ ] `simulate(ops-page, {}, 0)` gives the two §13.2 notifications
- [ ] `simulate(pay-page, schedules, 1710230400000)` gives the four §13.7 notifications, and with `stop=1710232200000` the first three
- [ ] `on_call_users(pay-page, schedules, 1710232200000) == frozenset({"dan", "lead"})` and `responder(pay-page, schedules, 1710230400000) == "dan"`
- [ ] `escalate` returns `[]` for a merged incident or unknown policy and uses `responded_at` before the other stop candidates; a notification at exactly `stop` is not sent
- [ ] `check_references` of the §11.6 rules against the §13.7 policies is `[(9, "pay-low"), (21, "dba-page")]`
- [ ] a level with both `schedule` and `users`, an unknown schedule, `delay_minutes: -1`, `repeat: false`, and empty `levels` each raise PolicyError

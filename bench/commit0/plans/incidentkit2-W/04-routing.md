# 04: Routing rules

**What to build:** Implement `incidentkit/routing.py` per SPEC §11 (lines 734–934):
`glob_match` (whole-string, `*`/`?` only, everything else literal and case-sensitive),
`parse_rules(text)` with the full line grammar and the ordered validation errors of
§11.4 (first error by line, with the "no team" error reported at the `route` line but
detected at `end`, unterminated block reported at its `route` line), the matchers
`service`, `source`, `severity` RANGE (`A`, `A..B`, `A..`, `..B`), `attr KEY OP VALUE`
(`!=` matches a missing attr), `message /REGEX/FLAGS` (last `/` ends the regex, flags
`""` or `"i"`), `team`/`policy`/`continue` once each; `Rule.matches(event)`,
`load_rules(path)` (UTF-8, OSError propagates), `routes_for` and `route`. Keep the tracer's
field names/order for `Route`, `Rule`, `RuleSet`; matcher storage is free.

**Blocked by:** 01

**Status:** ready-for-agent

**Files:** incidentkit/routing.py

**Context:**
`SPEC.md:738  §11.1 glob_match(pattern, text) -> bool`
`SPEC.md:751  §11.2 file grammar and statement table`
`SPEC.md:783  §11.3 matchers`
`SPEC.md:813  §11.4 validation errors and examples`
`SPEC.md:837  §11.5 Route(team, policy, rule), Rule(name, line, team, policy, cont, ...).matches(event), RuleSet(rules, default, default_line), parse_rules, load_rules, routes_for, route`
`SPEC.md:863  §11.6 worked example table`
`incidentkit/errors.py  RoutingError(line, reason)  — validation errors`
`incidentkit/model.py  Severity.parse(text); Event.attr(key, default=None)`

- [ ] every glob_match example of §11.1 holds, including `glob_match("a[1]", "a[1]") is True`
- [ ] parsing the §11.6 file gives 4 rules in order, `default == Route("ops", "ops-page", None)`, `default_line == 27`
- [ ] each row of the §11.6 table gives exactly the listed `routes_for` result; `route()` is its first element
- [ ] the three §11.4 examples raise RoutingError at lines 2, 1 and 4
- [ ] `severity ..`, `severity high..crit..`, `severity critical..low`, `attr k ~ v`, `message /x/g`, `message /(/`, a second `default`, `end` at top level, and EOF inside a block each raise RoutingError at the specified line
- [ ] without a default statement, `default == Route("unrouted", None, None)` and `default_line is None`

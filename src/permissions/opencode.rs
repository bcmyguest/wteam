//! Translate a profile for opencode: the `OPENCODE_PERMISSION` env JSON.

use anyhow::Result;

use super::{Decision, Profile, RuleSupport, Support, Translation, rules_of, tool_pattern};

/// `(claude tool, opencode key)`.
const KEYS: [(&str, &str); 4] = [
    ("Bash", "bash"),
    ("Read", "read"),
    ("Edit", "edit"),
    ("WebFetch", "webfetch"),
];

fn word(d: Decision) -> &'static str {
    match d {
        Decision::Allow => "allow",
        Decision::Ask => "ask",
        Decision::Deny => "deny",
    }
}

/// Strictness for deny-first resolution: deny beats ask beats allow.
fn rank(d: Decision) -> u8 {
    match d {
        Decision::Allow => 0,
        Decision::Ask => 1,
        Decision::Deny => 2,
    }
}

fn quote(s: &str) -> String {
    serde_json::Value::String(s.to_string()).to_string()
}

pub fn translate(profile: &Profile, role: &str) -> Result<Translation> {
    let _ = role;
    let rules = rules_of(profile)?;
    let mut support = Vec::new();
    // Per key: catch-all decision (default deny, strictest wins) and ordered entries.
    let mut catch_all = [Decision::Deny; 4];
    let mut entries: [Vec<(String, Decision)>; 4] = Default::default();
    let mut explicit = [false; 4];
    for (d, rendered, rule) in &rules {
        let slot = tool_pattern(rule)
            .and_then(|(tool, pat)| KEYS.iter().position(|(t, _)| *t == tool).map(|i| (i, pat)));
        match slot {
            Some((i, pat)) => {
                if pat == "*" || pat == "**" {
                    // Later groups (ask, deny) are stricter, so they overwrite.
                    if !explicit[i] || *d != Decision::Allow {
                        catch_all[i] = *d;
                        explicit[i] = true;
                    }
                } else {
                    entries[i].push((pat, *d));
                }
                support.push(RuleSupport {
                    decision: *d,
                    rule: rendered.clone(),
                    support: Support::Exact,
                });
            }
            None => support.push(RuleSupport {
                decision: *d,
                rule: rendered.clone(),
                support: Support::Unsupported(
                    "tool has no opencode permission key; ignored".into(),
                ),
            }),
        }
    }
    if profile.bypass {
        return Ok(Translation {
            args: vec!["--auto".into()],
            support,
            ..Translation::default()
        });
    }
    let mut parts = Vec::new();
    for (i, (_, key)) in KEYS.iter().enumerate() {
        let mut body = vec![format!("{}:{}", quote("*"), quote(word(catch_all[i])))];
        // Claude resolves deny-first, so an explicit catch-all beats any weaker specific
        // rule; drop those, or the last-match-wins order below would let them through.
        let mut ordered: Vec<&(String, Decision)> = entries[i]
            .iter()
            .filter(|(_, d)| !explicit[i] || rank(*d) >= rank(catch_all[i]))
            .collect();
        // Opencode applies the last matching rule: denies go last, asks before them.
        ordered.sort_by_key(|(_, d)| rank(*d));
        for (pat, d) in ordered {
            body.push(format!("{}:{}", quote(pat), quote(word(*d))));
        }
        parts.push(format!("{}:{{{}}}", quote(key), body.join(",")));
    }
    Ok(Translation {
        env: vec![(
            "OPENCODE_PERMISSION".into(),
            format!("{{{}}}", parts.join(",")),
        )],
        support,
        ..Translation::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permissions::builtin;

    #[test]
    fn edit_profile_golden() {
        let t = translate(&builtin("edit").unwrap(), "worker").unwrap();
        assert!(t.args.is_empty());
        assert_eq!(
            t.env,
            [(
                "OPENCODE_PERMISSION".to_string(),
                r#"{"bash":{"*":"deny","wteam *":"allow","git diff *":"allow","git status *":"allow","cargo *":"allow","npm *":"allow","pytest *":"allow","cd *":"allow","ls *":"allow","cat *":"allow","head *":"allow","tail *":"allow","grep *":"allow","rg *":"allow","wc *":"allow","pwd":"allow","python3 -m pytest *":"allow","go test *":"allow","make test":"allow","make check":"allow","git push *":"ask","rm -rf *":"deny"},"read":{"*":"allow","**/.env":"deny"},"edit":{"*":"allow"},"webfetch":{"*":"deny"}}"#
                    .to_string()
            )]
        );
        assert_eq!(t.support.len(), 24);
        assert!(t.support.iter().all(|s| s.support == Support::Exact));
        let v: serde_json::Value = serde_json::from_str(&t.env[0].1).unwrap();
        assert_eq!(v["read"]["**/.env"], "deny");
    }

    fn bash_map(p: &Profile) -> serde_json::Value {
        let t = translate(p, "w").unwrap();
        let v: serde_json::Value = serde_json::from_str(&t.env[0].1).unwrap();
        v["bash"].clone()
    }

    #[test]
    fn catch_all_deny_beats_specific_allow() {
        let p = Profile {
            name: "x".into(),
            allow: vec!["Bash(git status *)".into()],
            deny: vec!["Bash(*)".into()],
            ..Profile::default()
        };
        let t = translate(&p, "w").unwrap();
        assert!(
            t.env[0].1.contains(r#""bash":{"*":"deny"}"#),
            "{}",
            t.env[0].1
        );
        assert!(t.support.iter().all(|s| s.support == Support::Exact));
    }

    #[test]
    fn catch_all_ask_beats_allow_but_not_deny() {
        let p = Profile {
            name: "x".into(),
            allow: vec!["Bash(git status *)".into()],
            ask: vec!["Bash(*)".into()],
            deny: vec!["Bash(rm *)".into()],
            ..Profile::default()
        };
        assert_eq!(
            bash_map(&p),
            serde_json::json!({"*": "ask", "rm *": "deny"})
        );
    }

    #[test]
    fn pattern_with_quote_is_escaped() {
        let p = Profile {
            name: "x".into(),
            allow: vec![r#"Bash(echo "hi" *)"#.into()],
            ..Profile::default()
        };
        let t = translate(&p, "w").unwrap();
        assert!(t.env[0].1.contains(r#""echo \"hi\" *":"allow""#));
        assert_eq!(bash_map(&p)[r#"echo "hi" *"#], "allow");
    }

    #[test]
    fn read_only_profile_denies_edit() {
        let t = translate(&builtin("review").unwrap(), "reviewer").unwrap();
        assert!(
            t.env[0].1.contains(r#""edit":{"*":"deny"}"#),
            "{}",
            t.env[0].1
        );
    }

    #[test]
    fn bypass_golden() {
        let t = translate(&builtin("full").unwrap(), "worker").unwrap();
        assert_eq!(t.args, ["--auto"]);
        assert!(t.env.is_empty());
    }

    #[test]
    fn unknown_bare_tool_unsupported() {
        let p = Profile {
            name: "x".into(),
            allow: vec!["Glob".into()],
            ..Profile::default()
        };
        let t = translate(&p, "w").unwrap();
        assert!(matches!(t.support[0].support, Support::Unsupported(_)));
    }
}

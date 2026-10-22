//! Translate a profile for codex: execpolicy `prefix_rule`s plus a sandbox level.

use std::path::PathBuf;

use anyhow::Result;

use super::{Decision, Profile, Rule, RuleSupport, Support, Translation, rules_of, tool_pattern};

/// `(prefix tokens, exact)` for a Bash pattern: tokens before the first one holding `*`.
/// A lone trailing `*` is exact because a prefix rule already matches any arguments.
fn prefix(pattern: &str) -> (Vec<String>, bool) {
    let mut tokens = Vec::new();
    let all: Vec<&str> = pattern.split_whitespace().collect();
    for (i, t) in all.iter().enumerate() {
        if t.contains('*') {
            let exact = *t == "*" && i + 1 == all.len();
            return (tokens, exact);
        }
        tokens.push(t.to_string());
    }
    (tokens, true)
}

fn quote(s: &str) -> String {
    serde_json::Value::String(s.to_string()).to_string()
}

pub fn translate(profile: &Profile, role: &str) -> Result<Translation> {
    let rules = rules_of(profile)?;
    let mut lines = Vec::new();
    let mut support = Vec::new();
    let edit_allowed = rules
        .iter()
        .any(|(d, _, r)| *d == Decision::Allow && matches!(tool_pattern(r), Some(("Edit", _))));
    // Agents must run `wteam`, which a read-only sandbox blocks.
    let wteam_allowed = rules.iter().any(|(d, _, r)| {
        *d == Decision::Allow
            && tool_pattern(r).is_some_and(|(tool, p)| tool == "Bash" && p.starts_with("wteam"))
    });
    let level = if edit_allowed || wteam_allowed {
        "workspace-write"
    } else {
        "read-only"
    };
    for (d, rendered, rule) in &rules {
        let sup = match (rule, tool_pattern(rule)) {
            (_, Some(("Bash", pat))) => {
                let (tokens, exact) = prefix(&pat);
                if tokens.is_empty() {
                    Support::Unsupported(format!("no literal prefix; `{rendered}` not written"))
                } else {
                    let decision = match d {
                        Decision::Allow => "allow",
                        Decision::Ask => "prompt",
                        Decision::Deny => "forbidden",
                    };
                    let pattern: Vec<String> = tokens.iter().map(|t| quote(t)).collect();
                    lines.push(format!(
                        "prefix_rule(pattern=[{}], decision={})",
                        pattern.join(", "),
                        quote(decision)
                    ));
                    if exact {
                        Support::Exact
                    } else {
                        Support::Lossy(format!("prefix `{}`", tokens.join(" ")))
                    }
                }
            }
            (_, Some(("Read", _))) if *d == Decision::Deny => Support::Unsupported(
                "read denies are not enforced; the sandbox can read everything".into(),
            ),
            (_, Some(("Read" | "Edit", _))) => Support::Lossy(format!("sandbox {level}")),
            (Rule::WebFetch(_), _) | (_, Some(("WebFetch", _))) => {
                Support::Unsupported("not enforced; sandbox network setting applies".into())
            }
            _ => Support::Unsupported("tool has no codex equivalent; ignored".into()),
        };
        support.push(RuleSupport {
            decision: *d,
            rule: rendered.clone(),
            support: sup,
        });
    }
    if profile.bypass {
        return Ok(Translation {
            args: vec!["--dangerously-bypass-approvals-and-sandbox".into()],
            support,
            ..Translation::default()
        });
    }
    let mut files = Vec::new();
    if !lines.is_empty() {
        let mut text = lines.join("\n");
        text.push('\n');
        files.push((
            PathBuf::from(format!(".codex/rules/wteam-{role}.rules")),
            text,
        ));
    }
    Ok(Translation {
        // `-c` rather than `-s`: `codex exec resume` rejects `--sandbox`.
        args: vec!["-c".into(), format!("sandbox_mode={}", quote(level))],
        files,
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
        assert_eq!(t.args, ["-c", r#"sandbox_mode="workspace-write""#]);
        assert_eq!(t.files.len(), 1);
        assert_eq!(
            t.files[0].0,
            PathBuf::from(".codex/rules/wteam-worker.rules")
        );
        assert_eq!(
            t.files[0].1,
            r#"prefix_rule(pattern=["wteam"], decision="allow")
prefix_rule(pattern=["git", "diff"], decision="allow")
prefix_rule(pattern=["git", "status"], decision="allow")
prefix_rule(pattern=["mkdir"], decision="allow")
prefix_rule(pattern=["cargo"], decision="allow")
prefix_rule(pattern=["npm"], decision="allow")
prefix_rule(pattern=["pytest"], decision="allow")
prefix_rule(pattern=["cd"], decision="allow")
prefix_rule(pattern=["ls"], decision="allow")
prefix_rule(pattern=["cat"], decision="allow")
prefix_rule(pattern=["head"], decision="allow")
prefix_rule(pattern=["tail"], decision="allow")
prefix_rule(pattern=["grep"], decision="allow")
prefix_rule(pattern=["rg"], decision="allow")
prefix_rule(pattern=["wc"], decision="allow")
prefix_rule(pattern=["pwd"], decision="allow")
prefix_rule(pattern=["python3", "-m", "pytest"], decision="allow")
prefix_rule(pattern=["go", "test"], decision="allow")
prefix_rule(pattern=["make", "test"], decision="allow")
prefix_rule(pattern=["make", "check"], decision="allow")
prefix_rule(pattern=["python3"], decision="allow")
prefix_rule(pattern=["python"], decision="allow")
prefix_rule(pattern=["node"], decision="allow")
prefix_rule(pattern=["go"], decision="allow")
prefix_rule(pattern=["git", "push"], decision="prompt")
prefix_rule(pattern=["rm", "-rf"], decision="forbidden")
"#
        );
        assert_eq!(t.support.len(), 29);
        assert_eq!(
            t.support[0].support,
            Support::Lossy("sandbox workspace-write".into())
        );
        assert_eq!(t.support[2].support, Support::Exact);
    }

    #[test]
    fn review_profile_stays_writable_for_wteam() {
        let t = translate(&builtin("review").unwrap(), "reviewer").unwrap();
        assert_eq!(t.args, ["-c", r#"sandbox_mode="workspace-write""#]);
    }

    #[test]
    fn profile_without_edit_or_wteam_is_read_only() {
        let p = Profile {
            name: "x".into(),
            allow: vec!["Read(**)".into()],
            ..Profile::default()
        };
        let t = translate(&p, "w").unwrap();
        assert_eq!(t.args, ["-c", r#"sandbox_mode="read-only""#]);
    }

    #[test]
    fn glob_after_prefix_is_lossy() {
        let p = Profile {
            name: "x".into(),
            allow: vec![
                "Bash(cargo * --release)".into(),
                "Bash(*)".into(),
                "WebFetch(domain:x.com)".into(),
            ],
            ..Profile::default()
        };
        let t = translate(&p, "w").unwrap();
        assert_eq!(
            t.files[0].1,
            "prefix_rule(pattern=[\"cargo\"], decision=\"allow\")\n"
        );
        assert_eq!(
            t.support[0].support,
            Support::Lossy("prefix `cargo`".into())
        );
        assert!(matches!(t.support[1].support, Support::Unsupported(_)));
        assert!(matches!(t.support[2].support, Support::Unsupported(_)));
    }

    #[test]
    fn read_deny_is_unsupported() {
        let t = translate(&builtin("edit").unwrap(), "worker").unwrap();
        let deny = t
            .support
            .iter()
            .find(|s| s.rule == "Read(**/.env)")
            .unwrap();
        assert!(matches!(deny.support, Support::Unsupported(_)));
        let allow = t.support.iter().find(|s| s.rule == "Read(**)").unwrap();
        assert!(matches!(allow.support, Support::Lossy(_)));
    }

    #[test]
    fn bypass_golden() {
        let t = translate(&builtin("full").unwrap(), "worker").unwrap();
        assert_eq!(t.args, ["--dangerously-bypass-approvals-and-sandbox"]);
        assert!(t.files.is_empty());
    }
}

//! Translate a profile for claude: `--settings <json>` plus a permission mode.

use std::path::Path;

use anyhow::Result;
use serde_json::json;

use super::{Profile, Rule, RuleSupport, Support, Translation, parse_rule, rules_of};

/// Translate `profile` with path-scoped rules relative (the `wteam permissions` preview form).
pub fn translate(profile: &Profile, role: &str) -> Result<Translation> {
    translate_in(profile, role, None)
}

/// Translate `profile`. With `dir`, each relative path-scoped `allow` and `ask` rule is anchored
/// under `dir` in Claude's absolute form (`//<dir>/<pattern>`), so it holds after the agent `cd`s.
/// A relative path-scoped `deny` rule keeps its original form and gains the anchored one; `**`
/// and absolute rules stay as written. The support rows keep the profile's rule text.
pub fn translate_in(profile: &Profile, role: &str, dir: Option<&Path>) -> Result<Translation> {
    let _ = role;
    let rules = rules_of(profile)?;
    let support = rules
        .iter()
        .map(|(d, rendered, _)| RuleSupport {
            decision: *d,
            rule: rendered.clone(),
            support: Support::Exact,
        })
        .collect();
    if profile.bypass {
        return Ok(Translation {
            args: vec!["--dangerously-skip-permissions".into()],
            support,
            ..Translation::default()
        });
    }
    let settings = json!({
        "permissions": {
            "allow": anchored(&profile.allow, dir, false)?,
            "ask": anchored(&profile.ask, dir, false)?,
            "deny": anchored(&profile.deny, dir, true)?,
        }
    });
    Ok(Translation {
        args: vec![
            "--settings".into(),
            settings.to_string(),
            "--permission-mode".into(),
            "dontAsk".into(),
        ],
        support,
        ..Translation::default()
    })
}

/// One list of a profile in emitted order. With `dir`, a relative path-scoped rule is replaced by
/// its anchored form. In a deny list the original stays too: a deny that only matches under `dir`
/// must not weaken the original. Every other rule passes through unchanged.
fn anchored(list: &[String], dir: Option<&Path>, deny: bool) -> Result<Vec<String>> {
    let mut out = Vec::with_capacity(list.len());
    for rule in list {
        match dir.map(|d| anchor(rule, d)).transpose()?.flatten() {
            Some(anchored) => {
                if deny {
                    out.push(rule.clone());
                }
                out.push(anchored);
            }
            None => out.push(rule.clone()),
        }
    }
    Ok(out)
}

/// The anchored form of a relative `Read(<pattern>)` or `Edit(<pattern>)` under `dir`, or `None`
/// for any rule that stays as written.
fn anchor(rule: &str, dir: &Path) -> Result<Option<String>> {
    let base = dir.to_string_lossy();
    let under = |p: &str| format!("//{}/{p}", base.trim_matches('/'));
    Ok(match parse_rule(rule)? {
        Rule::Edit(p) if is_relative(&p) => Some(format!("Edit({})", under(&p))),
        Rule::Read(p) if is_relative(&p) => Some(format!("Read({})", under(&p))),
        _ => None,
    })
}

/// Relative and path-scoped: not `**`-prefixed (matches anywhere) and not absolute.
fn is_relative(pattern: &str) -> bool {
    !pattern.starts_with("**") && !pattern.starts_with('/')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permissions::{Decision, builtin};

    #[test]
    fn control_profile_golden() {
        let t = translate(&builtin("control").unwrap(), "worker").unwrap();
        assert_eq!(
            t.args,
            [
                "--settings",
                r#"{"permissions":{"allow":["Bash(wteam *)","Bash(git status *)","Bash(git log *)","Bash(git diff *)","Read(**)","Bash(cd *)","Bash(ls *)","Bash(cat *)","Bash(head *)","Bash(tail *)","Bash(grep *)","Bash(rg *)","Bash(wc *)","Bash(pwd)"],"ask":[],"deny":["Edit(**)"]}}"#,
                "--permission-mode",
                "dontAsk",
            ]
        );
        assert!(t.env.is_empty() && t.files.is_empty());
        assert_eq!(t.support.len(), 15);
        assert!(t.support.iter().all(|s| s.support == Support::Exact));
        assert_eq!(t.support[14].decision, Decision::Deny);
        assert_eq!(t.support[14].rule, "Edit(**)");
    }

    #[test]
    fn bypass_golden() {
        let t = translate(&builtin("full").unwrap(), "worker").unwrap();
        assert_eq!(t.args, ["--dangerously-skip-permissions"]);
        assert!(t.support.is_empty());
    }

    #[test]
    fn plan_profile_allows_only_wteam_edits() {
        let t = translate(&builtin("plan").unwrap(), "planner").unwrap();
        let settings: serde_json::Value = serde_json::from_str(&t.args[1]).unwrap();
        let allow = settings["permissions"]["allow"].as_array().unwrap();
        assert!(allow.iter().any(|r| r == "Edit(.wteam/**)"));
        assert!(!allow.iter().any(|r| r == "Edit(**)"));
        assert!(
            settings["permissions"]["deny"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        // dontAsk denies every edit that no allow rule names.
        assert_eq!(t.args[3], "dontAsk");
    }

    #[test]
    fn review_profile_golden() {
        let t = translate(&builtin("review").unwrap(), "reviewer").unwrap();
        assert_eq!(
            t.args,
            [
                "--settings",
                r#"{"permissions":{"allow":["Read(**)","Bash(wteam *)","Bash(git diff *)","Bash(git log *)","Bash(cargo test *)","Bash(npm test *)","Bash(pytest *)","Bash(cd *)","Bash(ls *)","Bash(cat *)","Bash(head *)","Bash(tail *)","Bash(grep *)","Bash(rg *)","Bash(wc *)","Bash(pwd)","Bash(python3 -m pytest *)","Bash(go test *)","Bash(make test)","Bash(make check)","Bash(python3 *)","Bash(python *)","Bash(node *)","Bash(go *)","Bash(cargo *)","Bash(npm *)","Edit(scratch/**)"],"ask":[],"deny":[]}}"#,
                "--permission-mode",
                "dontAsk",
            ]
        );
        assert!(t.support.iter().all(|s| s.support == Support::Exact));
        assert!(!t.args[1].contains("Edit(**)"));
    }

    fn settings_of(t: &Translation) -> serde_json::Value {
        serde_json::from_str::<serde_json::Value>(&t.args[1]).unwrap()["permissions"].clone()
    }

    #[test]
    fn planner_edit_rule_anchors_under_repo_root() {
        let t = translate_in(
            &builtin("plan").unwrap(),
            "planner",
            Some(Path::new("/tmp/repo")),
        )
        .unwrap();
        let allow = &settings_of(&t)["allow"];
        assert!(
            allow
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r == "Edit(//tmp/repo/.wteam/**)")
        );
        assert!(
            !allow
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r == "Edit(.wteam/**)")
        );
        assert!(allow.as_array().unwrap().iter().any(|r| r == "Read(**)"));
        // Preview rows keep the profile's relative text.
        assert!(t.support.iter().any(|s| s.rule == "Edit(.wteam/**)"));
    }

    #[test]
    fn reviewer_scratch_rule_anchors_under_pair_worktree() {
        let dir = Path::new("/home/b/wt/pair1");
        let t = translate_in(&builtin("review").unwrap(), "reviewer", Some(dir)).unwrap();
        let allow = settings_of(&t)["allow"].clone();
        assert!(
            allow
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r == "Edit(//home/b/wt/pair1/scratch/**)")
        );
        assert!(
            !allow
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r == "Edit(scratch/**)")
        );
    }

    #[test]
    fn unscoped_and_absolute_rules_stay_put() {
        let p = Profile {
            name: "t".into(),
            allow: vec![
                "Read(**)".into(),
                "Edit(**)".into(),
                "Edit(//etc/x/**)".into(),
                "Bash(git diff *)".into(),
                "Edit(scratch/**)".into(),
            ],
            ask: vec![],
            deny: vec!["Read(**/.env)".into()],
            bypass: false,
        };
        let t = translate_in(&p, "worker", Some(Path::new("/tmp/repo"))).unwrap();
        let s = settings_of(&t);
        assert_eq!(
            s["allow"],
            json!([
                "Read(**)",
                "Edit(**)",
                "Edit(//etc/x/**)",
                "Bash(git diff *)",
                "Edit(//tmp/repo/scratch/**)"
            ])
        );
        // Deny stays as written: anchoring it would leave `.env` files outside the dir readable.
        assert_eq!(s["deny"], json!(["Read(**/.env)"]));
    }

    #[test]
    fn no_dir_keeps_relative_rules() {
        let t = translate(&builtin("plan").unwrap(), "planner").unwrap();
        assert!(t.args[1].contains("Edit(.wteam/**)"));
    }

    #[test]
    fn relative_deny_is_emitted_in_both_forms() {
        let p = Profile {
            name: "t".into(),
            allow: vec![],
            ask: vec![],
            deny: vec!["Edit(secrets/**)".into(), "Read(**/.env)".into()],
            bypass: false,
        };
        let t = translate_in(&p, "worker", Some(Path::new("/tmp/repo"))).unwrap();
        assert_eq!(
            settings_of(&t)["deny"],
            json!([
                "Edit(secrets/**)",
                "Edit(//tmp/repo/secrets/**)",
                "Read(**/.env)"
            ])
        );
    }

    #[test]
    fn trailing_slash_on_dir_anchors_once() {
        let t = translate_in(
            &builtin("plan").unwrap(),
            "planner",
            Some(Path::new("/tmp/repo/")),
        )
        .unwrap();
        let allow = settings_of(&t)["allow"].clone();
        assert!(
            allow
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r == "Edit(//tmp/repo/.wteam/**)")
        );
        assert!(!t.args[1].contains("//tmp/repo//"));
    }
}

//! Translate a profile for claude: `--settings <json>` plus a permission mode.

use anyhow::Result;
use serde_json::json;

use super::{Profile, RuleSupport, Support, Translation, rules_of};

pub fn translate(profile: &Profile, role: &str) -> Result<Translation> {
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
            "allow": profile.allow,
            "ask": profile.ask,
            "deny": profile.deny,
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
}

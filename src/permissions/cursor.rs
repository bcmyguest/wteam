//! Translate a profile for cursor: `.cursor/cli.json` allow/deny lists.

use std::path::PathBuf;

use anyhow::Result;
use serde_json::json;

use super::{Decision, Profile, Rule, RuleSupport, Support, Translation, rules_of, tool_pattern};

/// A rule in cursor syntax: `Shell(cmd)`, `Read(glob)`, `Write(glob)`, `WebFetch(domain)`.
fn cursor_rule(rule: &Rule) -> Option<String> {
    let (tool, pat) = tool_pattern(rule)?;
    Some(match tool {
        "Bash" => format!("Shell({pat})"),
        "Read" => format!("Read({pat})"),
        "Edit" => format!("Write({pat})"),
        _ => format!("WebFetch({})", pat.strip_prefix("domain:").unwrap_or(&pat)),
    })
}

pub fn translate(profile: &Profile, role: &str) -> Result<Translation> {
    let _ = role;
    let mut allow = Vec::new();
    let mut deny = Vec::new();
    let mut support = Vec::new();
    for (d, rendered, rule) in rules_of(profile)? {
        let sup = match (cursor_rule(&rule), d) {
            (None, _) => Support::Unsupported("tool has no cursor permission; ignored".into()),
            (Some(r), Decision::Allow) => {
                allow.push(r);
                Support::Exact
            }
            (Some(r), Decision::Deny) => {
                deny.push(r);
                Support::Exact
            }
            (Some(r), Decision::Ask) => {
                deny.push(r);
                Support::Lossy("deny (cursor has no ask)".into())
            }
        };
        support.push(RuleSupport {
            decision: d,
            rule: rendered,
            support: sup,
        });
    }
    if profile.bypass {
        return Ok(Translation {
            args: vec!["--force".into()],
            support,
            ..Translation::default()
        });
    }
    let body = json!({ "permissions": { "allow": allow, "deny": deny } });
    let mut text = serde_json::to_string_pretty(&body)?;
    text.push('\n');
    Ok(Translation {
        files: vec![(PathBuf::from(".cursor/cli.json"), text)],
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
        assert!(t.args.is_empty() && t.env.is_empty());
        assert_eq!(t.files.len(), 1);
        assert_eq!(t.files[0].0, PathBuf::from(".cursor/cli.json"));
        assert_eq!(
            t.files[0].1,
            r#"{
  "permissions": {
    "allow": [
      "Read(**)",
      "Write(**)",
      "Shell(wteam *)",
      "Shell(git diff *)",
      "Shell(git status *)",
      "Shell(cargo *)",
      "Shell(npm *)",
      "Shell(pytest *)",
      "Shell(cd *)",
      "Shell(ls *)",
      "Shell(cat *)",
      "Shell(head *)",
      "Shell(tail *)",
      "Shell(grep *)",
      "Shell(rg *)",
      "Shell(wc *)",
      "Shell(pwd)",
      "Shell(python3 -m pytest *)",
      "Shell(go test *)",
      "Shell(make test)",
      "Shell(make check)"
    ],
    "deny": [
      "Shell(git push *)",
      "Read(**/.env)",
      "Shell(rm -rf *)"
    ]
  }
}
"#
        );
        assert_eq!(t.support.len(), 24);
        let ask = t
            .support
            .iter()
            .find(|s| s.decision == Decision::Ask)
            .unwrap();
        assert_eq!(
            ask.support,
            Support::Lossy("deny (cursor has no ask)".into())
        );
        assert_eq!(
            t.support
                .iter()
                .filter(|s| s.support == Support::Exact)
                .count(),
            23
        );
    }

    #[test]
    fn web_fetch_strips_domain_prefix() {
        let p = Profile {
            name: "x".into(),
            allow: vec!["WebFetch(domain:example.com)".into()],
            ..Profile::default()
        };
        let t = translate(&p, "w").unwrap();
        assert!(t.files[0].1.contains("\"WebFetch(example.com)\""));
    }

    #[test]
    fn bypass_golden() {
        let t = translate(&builtin("full").unwrap(), "worker").unwrap();
        assert_eq!(t.args, ["--force"]);
        assert!(t.files.is_empty());
    }
}

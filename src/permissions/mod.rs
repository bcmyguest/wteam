//! Permission profiles in Claude Code's rule syntax, translated per agent CLI.
//!
//! A profile is TOML:
//!
//! ```toml
//! name = "edit"
//! allow = ["Read(**)", "Edit(src/**)", "Bash(cargo test *)"]
//! ask = ["Bash(git push *)"]
//! deny = ["Read(**/.env)", "Bash(rm -rf *)"]
//! ```
//!
//! Each translator reports, per rule, whether the CLI honours it exactly, lossily
//! (with what it becomes), or not at all (with the fallback). `wteam permissions`
//! prints that as the ✔/≈/✘ table users confirm before a run.

pub mod claude;
pub mod codex;
pub mod cursor;
pub mod opencode;
pub mod pi;

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

/// One rule. `Tool` is a bare tool name (e.g. `Read`) matching every use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rule {
    Bash(String),
    Read(String),
    Edit(String),
    WebFetch(String),
    Tool(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Allow,
    Ask,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub ask: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
    /// `true` only for the `full` profile: pass the CLI's bypass flag.
    #[serde(default)]
    pub bypass: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "support", content = "detail", rename_all = "snake_case")]
pub enum Support {
    Exact,
    /// Honoured approximately; the string says what it becomes.
    Lossy(String),
    /// Not honoured; the string names the fallback.
    Unsupported(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuleSupport {
    pub decision: Decision,
    pub rule: String,
    pub support: Support,
}

/// What a CLI needs to enforce a profile.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct Translation {
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    /// Files to write, relative to the repo root (backed up first).
    pub files: Vec<(PathBuf, String)>,
    pub support: Vec<RuleSupport>,
}

/// Built-in profile names, in display order.
pub const BUILTINS: [&str; 5] = ["control", "review", "edit", "plan", "full"];

/// CLIs `wteam permissions` shows by default.
pub const DEFAULT_CLIS: [&str; 5] = ["claude", "codex", "opencode", "pi", "cursor"];

/// Parse `Bash(git commit *)`, `Read(src/**)`, `Edit(x)`, `WebFetch(domain:x)` or a bare tool name.
pub fn parse_rule(s: &str) -> Result<Rule> {
    let s = s.trim();
    let Some(open) = s.find('(') else {
        if s.is_empty() || s.contains(')') {
            bail!("malformed rule `{s}`");
        }
        return Ok(Rule::Tool(s.to_string()));
    };
    let Some(arg) = s[open + 1..].strip_suffix(')') else {
        bail!("malformed rule `{s}`: unbalanced parentheses");
    };
    if arg.contains(['(', ')']) && (arg.matches('(').count() != arg.matches(')').count()) {
        bail!("malformed rule `{s}`: unbalanced parentheses");
    }
    let arg = arg.to_string();
    match &s[..open] {
        "Bash" => Ok(Rule::Bash(arg)),
        "Read" => Ok(Rule::Read(arg)),
        "Edit" => Ok(Rule::Edit(arg)),
        "WebFetch" => Ok(Rule::WebFetch(arg)),
        other => bail!("malformed rule `{s}`: unknown tool `{other}` takes no arguments"),
    }
}

/// Render a rule back to Claude syntax.
pub fn render_rule(r: &Rule) -> String {
    match r {
        Rule::Bash(p) => format!("Bash({p})"),
        Rule::Read(p) => format!("Read({p})"),
        Rule::Edit(p) => format!("Edit({p})"),
        Rule::WebFetch(p) => format!("WebFetch({p})"),
        Rule::Tool(t) => t.clone(),
    }
}

fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

/// Built-in profiles: `control`, `review`, `edit`, `plan`, `full`.
pub fn builtin(name: &str) -> Option<Profile> {
    let (allow, ask, deny): (&[&str], &[&str], &[&str]) = match name {
        "control" => (
            &[
                "Bash(wteam *)",
                "Bash(git status *)",
                "Bash(git log *)",
                "Bash(git diff *)",
                "Read(**)",
                "Bash(cd *)",
                "Bash(ls *)",
                "Bash(cat *)",
                "Bash(head *)",
                "Bash(tail *)",
                "Bash(grep *)",
                "Bash(rg *)",
                "Bash(wc *)",
                "Bash(pwd)",
            ],
            &[],
            &["Edit(**)"],
        ),
        "review" => (
            // Reviewers write their checks and verdict under `scratch/` only. No `Edit(**)` deny:
            // claude and cursor resolve deny before allow, which would shadow `scratch/**`. Edits
            // outside `scratch/` are denied by default on claude, opencode and cursor.
            &[
                "Read(**)",
                "Bash(wteam *)",
                "Bash(git diff *)",
                "Bash(git log *)",
                "Bash(cargo test *)",
                "Bash(npm test *)",
                "Bash(pytest *)",
                "Bash(cd *)",
                "Bash(ls *)",
                "Bash(cat *)",
                "Bash(head *)",
                "Bash(tail *)",
                "Bash(grep *)",
                "Bash(rg *)",
                "Bash(wc *)",
                "Bash(pwd)",
                "Bash(python3 -m pytest *)",
                "Bash(go test *)",
                "Bash(make test)",
                "Bash(make check)",
                "Bash(python3 *)",
                "Bash(python *)",
                "Bash(node *)",
                "Bash(go *)",
                "Bash(cargo *)",
                "Bash(npm *)",
                "Edit(scratch/**)",
            ],
            &[],
            &[],
        ),
        "edit" => (
            &[
                "Read(**)",
                "Edit(**)",
                "Bash(wteam *)",
                "Bash(git diff *)",
                "Bash(git status *)",
                "Bash(mkdir *)",
                "Bash(cargo *)",
                "Bash(npm *)",
                "Bash(pytest *)",
                "Bash(cd *)",
                "Bash(ls *)",
                "Bash(cat *)",
                "Bash(head *)",
                "Bash(tail *)",
                "Bash(grep *)",
                "Bash(rg *)",
                "Bash(wc *)",
                "Bash(pwd)",
                "Bash(python3 -m pytest *)",
                "Bash(go test *)",
                "Bash(make test)",
                "Bash(make check)",
                "Bash(python3 *)",
                "Bash(python *)",
                "Bash(node *)",
                "Bash(go *)",
            ],
            &["Bash(git push *)"],
            &["Read(**/.env)", "Bash(rm -rf *)"],
        ),
        "plan" => (
            // The planner reads code, writes `.wteam/` and runs `wteam`. No interpreters or build
            // tools: they can write anywhere. No `Edit(**)` deny either: claude and cursor resolve
            // deny before allow, which would shadow the `.wteam` allow. Edits outside `.wteam/`
            // are denied by default on claude (`dontAsk`) and opencode (catch-all deny). Codex
            // maps any allowed edit to workspace-write, so it can edit the whole workspace.
            &[
                "Read(**)",
                "Edit(.wteam/**)",
                "Bash(wteam *)",
                "Bash(git status *)",
                "Bash(git log *)",
                "Bash(git diff *)",
                "Bash(ls *)",
                "Bash(cat *)",
                "Bash(head *)",
                "Bash(tail *)",
                "Bash(grep *)",
                "Bash(rg *)",
                "Bash(wc *)",
                "Bash(pwd)",
                "Bash(cd *)",
            ],
            &[],
            &[],
        ),
        "full" => {
            return Some(Profile {
                name: name.into(),
                bypass: true,
                ..Profile::default()
            });
        }
        _ => return None,
    };
    Some(Profile {
        name: name.into(),
        allow: strs(allow),
        ask: strs(ask),
        deny: strs(deny),
        bypass: false,
    })
}

/// Find profile `name` in `search` dirs (`<name>.toml`), else the built-ins.
pub fn load(name: &str, search: &[PathBuf]) -> Result<Profile> {
    if name.is_empty() || name.contains(['/', '\\']) || name.starts_with('.') {
        bail!("invalid profile name `{name}`");
    }
    for dir in search {
        let file = dir.join(format!("{name}.toml"));
        if !file.is_file() {
            continue;
        }
        let text = std::fs::read_to_string(&file)
            .with_context(|| format!("reading {}", file.display()))?;
        let mut table: toml::Table =
            toml::from_str(&text).with_context(|| format!("parsing {}", file.display()))?;
        table
            .entry("name")
            .or_insert_with(|| toml::Value::String(name.to_string()));
        let profile: Profile = table
            .try_into()
            .with_context(|| format!("parsing {}", file.display()))?;
        rules_of(&profile).with_context(|| format!("in {}", file.display()))?;
        return Ok(profile);
    }
    builtin(name).with_context(|| {
        format!(
            "unknown profile `{name}`; built-ins: {}",
            BUILTINS.join(", ")
        )
    })
}

/// Every rule of `p` parsed, in order allow, ask, deny.
pub fn rules_of(p: &Profile) -> Result<Vec<(Decision, String, Rule)>> {
    let groups = [
        (Decision::Allow, &p.allow),
        (Decision::Ask, &p.ask),
        (Decision::Deny, &p.deny),
    ];
    let mut out = Vec::new();
    for (d, list) in groups {
        for raw in list {
            let rule = parse_rule(raw)?;
            out.push((d, render_rule(&rule), rule));
        }
    }
    Ok(out)
}

/// A rule as `(tool, pattern)`; bare tools match everything. `None` for unknown bare tools.
pub(crate) fn tool_pattern(r: &Rule) -> Option<(&'static str, String)> {
    match r {
        Rule::Bash(p) => Some(("Bash", p.clone())),
        Rule::Read(p) => Some(("Read", p.clone())),
        Rule::Edit(p) => Some(("Edit", p.clone())),
        Rule::WebFetch(p) => Some(("WebFetch", p.clone())),
        Rule::Tool(t) => match t.as_str() {
            "Bash" => Some(("Bash", "*".into())),
            "Read" => Some(("Read", "**".into())),
            "Edit" => Some(("Edit", "**".into())),
            "WebFetch" => Some(("WebFetch", "*".into())),
            _ => None,
        },
    }
}

/// Translate `profile` for `cli` (`claude|codex|opencode|pi|cursor|fake`). `role` names generated files.
pub fn translate(cli: &str, profile: &Profile, role: &str) -> Result<Translation> {
    match cli {
        "claude" => claude::translate(profile, role),
        "codex" => codex::translate(profile, role),
        "opencode" => opencode::translate(profile, role),
        "pi" => pi::translate(profile, role),
        "cursor" => cursor::translate(profile, role),
        "fake" => Ok(Translation::default()),
        other => {
            bail!("unknown cli `{other}`; expected claude, codex, opencode, pi, cursor or fake")
        }
    }
}

/// Per-CLI support for every rule of `profile`: one `RuleSupport` per rule.
pub fn support_matrix(profile: &Profile, clis: &[&str]) -> Result<Vec<(String, Vec<RuleSupport>)>> {
    clis.iter()
        .map(|cli| {
            let t = translate(cli, profile, &profile.name)?;
            Ok((cli.to_string(), t.support))
        })
        .collect()
}

fn cell(s: &Support) -> String {
    match s {
        Support::Exact => "✔".into(),
        Support::Lossy(what) => format!("≈ {what}"),
        Support::Unsupported(fallback) => format!("✘ {fallback}"),
    }
}

/// The ✔/≈/✘ table: one row per rule, one column per CLI.
pub fn support_table(profile: &Profile, clis: &[&str]) -> Result<String> {
    let matrix = support_matrix(profile, clis)?;
    let rules = rules_of(profile)?;
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut header = vec!["decision".to_string(), "rule".to_string()];
    header.extend(clis.iter().map(|c| c.to_string()));
    rows.push(header);
    if profile.bypass {
        let mut row = vec!["bypass".to_string(), "(all permissions)".to_string()];
        row.extend(clis.iter().map(|_| "✔".to_string()));
        rows.push(row);
    }
    for (i, (d, rendered, _)) in rules.iter().enumerate() {
        let mut row = vec![format!("{d:?}").to_lowercase(), rendered.clone()];
        for (_, support) in &matrix {
            row.push(
                support
                    .get(i)
                    .map(|r| cell(&r.support))
                    .unwrap_or_else(|| "✔".into()),
            );
        }
        rows.push(row);
    }
    let cols = rows[0].len();
    let widths: Vec<usize> = (0..cols)
        .map(|c| rows.iter().map(|r| r[c].chars().count()).max().unwrap_or(0))
        .collect();
    let mut out = String::new();
    for row in &rows {
        let line: Vec<String> = row
            .iter()
            .zip(&widths)
            .map(|(v, w)| format!("{v}{}", " ".repeat(w - v.chars().count())))
            .collect();
        out.push_str(line.join("  ").trim_end());
        out.push('\n');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_round_trip() {
        for s in [
            "Bash(git commit *)",
            "Read(src/**)",
            "Edit(x)",
            "WebFetch(domain:example.com)",
            "Glob",
            "Bash(echo (hi))",
        ] {
            assert_eq!(render_rule(&parse_rule(s).unwrap()), s);
        }
        assert_eq!(parse_rule("Read").unwrap(), Rule::Tool("Read".into()));
    }

    #[test]
    fn interpreter_rules_let_agents_self_check() {
        let rules = [
            "Bash(python3 *)",
            "Bash(python *)",
            "Bash(node *)",
            "Bash(go *)",
            "Bash(cargo *)",
            "Bash(npm *)",
            "Bash(pytest *)",
        ];
        for name in ["review", "edit"] {
            let p = builtin(name).unwrap();
            for rule in rules {
                assert!(p.allow.iter().any(|a| a == rule), "{name} lacks {rule}");
            }
        }
    }

    #[test]
    fn malformed_rules_error() {
        for s in [
            "Bash(git",
            "Bash)",
            "Bash(a))",
            "Bash(a(b)",
            "Glob(x)",
            "",
            "(x)",
        ] {
            assert!(parse_rule(s).is_err(), "{s} should fail");
        }
    }

    #[test]
    fn builtins_match_contract() {
        let shell_basics = [
            "Bash(cd *)",
            "Bash(ls *)",
            "Bash(cat *)",
            "Bash(head *)",
            "Bash(tail *)",
            "Bash(grep *)",
            "Bash(rg *)",
            "Bash(wc *)",
            "Bash(pwd)",
        ];
        let runners = [
            "Bash(python3 -m pytest *)",
            "Bash(go test *)",
            "Bash(make test)",
            "Bash(make check)",
        ];
        // `find` writes (`-exec`, `-delete`), so no profile allows it.
        for name in ["control", "review", "edit"] {
            let p = builtin(name).unwrap();
            assert!(
                !p.allow.iter().any(|a| a == "Bash(find *)"),
                "{name} allows find"
            );
        }
        for name in ["control", "review", "edit"] {
            let p = builtin(name).unwrap();
            for rule in shell_basics {
                assert!(p.allow.iter().any(|a| a == rule), "{name} lacks {rule}");
            }
        }
        for name in ["review", "edit"] {
            let p = builtin(name).unwrap();
            for rule in runners {
                assert!(p.allow.iter().any(|a| a == rule), "{name} lacks {rule}");
            }
        }
        let control = builtin("control").unwrap();
        for rule in runners {
            assert!(
                !control.allow.iter().any(|a| a == rule),
                "control has {rule}"
            );
        }
        let edit = builtin("edit").unwrap();
        assert!(edit.allow.iter().any(|a| a == "Bash(mkdir *)"));
        assert_eq!(edit.ask, ["Bash(git push *)"]);
        assert_eq!(edit.deny, ["Read(**/.env)", "Bash(rm -rf *)"]);
        assert!(builtin("full").unwrap().bypass);
        assert!(builtin("nope").is_none());
        for n in BUILTINS {
            let p = builtin(n).unwrap();
            assert!(rules_of(&p).is_ok());
        }
    }

    #[test]
    fn load_prefers_search_dirs_over_builtins() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("edit.toml"),
            "name = \"edit\"\nallow = [\"Read(**)\"]\n",
        )
        .unwrap();
        let p = load("edit", &[tmp.path().to_path_buf()]).unwrap();
        assert_eq!(p.allow, ["Read(**)"]);
        assert!(p.deny.is_empty());
        let builtin_edit = load("edit", &[]).unwrap();
        assert_eq!(builtin_edit.ask, ["Bash(git push *)"]);
    }

    #[test]
    fn load_unknown_lists_builtins() {
        let err = load("nope", &[]).unwrap_err().to_string();
        assert!(err.contains("nope"), "{err}");
        for n in BUILTINS {
            assert!(err.contains(n), "{err}");
        }
    }

    #[test]
    fn load_rejects_malformed_rules() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("bad.toml"), "allow = [\"Bash(oops\"]\n").unwrap();
        assert!(load("bad", &[tmp.path().to_path_buf()]).is_err());
    }

    #[test]
    fn table_has_column_per_cli_and_symbols() {
        let p = Profile {
            name: "t".into(),
            allow: vec!["Bash(git diff *)".into(), "Edit(src/**)".into()],
            ask: vec!["Bash(git push *)".into()],
            deny: vec!["WebFetch(domain:x.com)".into()],
            bypass: false,
        };
        let t = support_table(&p, &["claude", "codex", "cursor"]).unwrap();
        let header = t.lines().next().unwrap();
        for c in ["claude", "codex", "cursor"] {
            assert!(header.contains(c), "{t}");
        }
        assert!(t.contains('✔'), "{t}");
        assert!(t.contains("≈ deny"), "{t}");
        assert!(t.contains("✘ "), "{t}");
        assert_eq!(t.lines().count(), 5, "{t}");
    }

    #[test]
    fn fake_translation_is_empty() {
        let p = builtin("edit").unwrap();
        assert_eq!(
            translate("fake", &p, "worker").unwrap(),
            Translation::default()
        );
        assert!(translate("bogus", &p, "worker").is_err());
    }

    #[test]
    fn plan_profile_reads_edits_wteam_and_runs_no_writers() {
        let plan = builtin("plan").unwrap();
        assert!(plan.deny.is_empty() && plan.ask.is_empty());
        assert!(plan.allow.iter().any(|a| a == "Edit(.wteam/**)"));
        assert!(!plan.allow.iter().any(|a| a == "Edit(**)"));
        // No interpreter or build tool: each can write outside `.wteam/`.
        for rule in [
            "python3 *",
            "python *",
            "node *",
            "go *",
            "cargo *",
            "npm *",
            "pytest *",
            "make test",
            "make check",
        ] {
            assert!(
                !plan.allow.iter().any(|a| a == &format!("Bash({rule})")),
                "plan allows {rule}"
            );
        }
        // Claude, opencode and cursor scope edits by path exactly; codex can only switch the whole
        // sandbox; pi enforces nothing.
        let expected = [
            ("claude", Support::Exact),
            ("opencode", Support::Exact),
            ("cursor", Support::Exact),
            ("codex", Support::Lossy("sandbox workspace-write".into())),
            ("pi", Support::Unsupported(pi::FALLBACK.into())),
        ];
        for (cli, want) in expected {
            let t = translate(cli, &plan, "planner").unwrap();
            let edit = t
                .support
                .iter()
                .find(|s| s.rule == "Edit(.wteam/**)")
                .unwrap_or_else(|| panic!("{cli} lacks the Edit row"));
            assert_eq!(edit.support, want, "{cli}");
        }
    }

    #[test]
    fn review_profile_edits_only_scratch() {
        let review = builtin("review").unwrap();
        assert!(review.deny.is_empty() && review.ask.is_empty());
        assert!(review.allow.iter().any(|a| a == "Edit(scratch/**)"));
        assert!(!review.allow.iter().any(|a| a == "Edit(**)"));
        // Same CLI split as `plan`: path-scoped on claude, opencode and cursor; codex switches
        // the whole sandbox; pi enforces nothing.
        let expected = [
            ("claude", Support::Exact),
            ("opencode", Support::Exact),
            ("cursor", Support::Exact),
            ("codex", Support::Lossy("sandbox workspace-write".into())),
            ("pi", Support::Unsupported(pi::FALLBACK.into())),
        ];
        for (cli, want) in expected {
            let t = translate(cli, &review, "reviewer").unwrap();
            let edit = t
                .support
                .iter()
                .find(|s| s.rule == "Edit(scratch/**)")
                .unwrap_or_else(|| panic!("{cli} lacks the Edit row"));
            assert_eq!(edit.support, want, "{cli}");
        }
    }
}

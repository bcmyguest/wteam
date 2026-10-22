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

use std::path::PathBuf;

use anyhow::{Result, bail};
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

/// Parse `Bash(git commit *)`, `Read(src/**)`, `Edit(x)`, `WebFetch(domain:x)` or a bare tool name.
pub fn parse_rule(s: &str) -> Result<Rule> {
    let _ = s;
    bail!("not implemented: permissions::parse_rule")
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

/// Built-in profiles: `control`, `review`, `edit`, `full`.
pub fn builtin(name: &str) -> Option<Profile> {
    let _ = name;
    None
}

/// Find profile `name` in `search` dirs (`<name>.toml`), else the built-ins.
pub fn load(name: &str, search: &[PathBuf]) -> Result<Profile> {
    let _ = (name, search);
    bail!("not implemented: permissions::load")
}

/// Translate `profile` for `cli` (`claude|codex|opencode|cursor|fake`). `role` names generated files.
pub fn translate(cli: &str, profile: &Profile, role: &str) -> Result<Translation> {
    let _ = (cli, profile, role);
    bail!("not implemented: permissions::translate")
}

/// The ✔/≈/✘ table: one row per rule, one column per CLI.
pub fn support_table(profile: &Profile, clis: &[&str]) -> Result<String> {
    let _ = (profile, clis);
    bail!("not implemented: permissions::support_table")
}

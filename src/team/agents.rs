//! Agent definitions: reusable TOML files naming a CLI, model, role prompt and
//! permission profile.
//!
//! ```toml
//! name = "qwen-worker"
//! cli = "opencode"            # claude | codex | opencode | cursor | pi | fake
//! model = "lemonade/qwen3.8-flash"
//! prompt = "worker"           # built-in role prompt name, or a path
//! profile = "edit"            # permission profile name
//! extra_args = []
//! ```

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentDef {
    pub name: String,
    pub cli: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub extra_args: Vec<String>,
    /// Context window in tokens. The orchestrator hands off at 75% of it when set.
    #[serde(default)]
    pub context_window: Option<u64>,
}

/// Built-in definitions: (name, model, profile), all on the `claude` CLI.
const BUILTINS: [(&str, &str, &str); 4] = [
    ("planner", "opus", "review"),
    ("orchestrator", "haiku", "control"),
    ("worker", "sonnet", "edit"),
    ("reviewer", "sonnet", "review"),
];

/// Find agent `name` in `search` dirs (first hit wins), else the built-ins.
pub fn load(name: &str, search: &[PathBuf]) -> Result<AgentDef> {
    for dir in search {
        let file = dir.join(format!("{name}.toml"));
        if file.is_file() {
            let raw = std::fs::read_to_string(&file)
                .with_context(|| format!("reading {}", file.display()))?;
            return toml::from_str(&raw).with_context(|| format!("parsing {}", file.display()));
        }
    }
    match BUILTINS.iter().find(|(n, _, _)| *n == name) {
        Some((n, model, profile)) => Ok(AgentDef {
            name: n.to_string(),
            cli: "claude".into(),
            model: Some(model.to_string()),
            prompt: Some(n.to_string()),
            profile: Some(profile.to_string()),
            extra_args: Vec::new(),
            context_window: None,
        }),
        None => bail!(
            "no agent definition `{name}` (searched {} dirs and the built-ins)",
            search.len()
        ),
    }
}

fn builtin_role(name: &str) -> Option<&'static str> {
    Some(match name {
        "planner" => include_str!("../../roles/planner.md"),
        "orchestrator" => include_str!("../../roles/orchestrator.md"),
        "worker" => include_str!("../../roles/worker.md"),
        "reviewer" => include_str!("../../roles/reviewer.md"),
        _ => return None,
    })
}

/// Role prompt text for `name`. `search` holds the agent dirs (`.wteam/agents`,
/// `$WTEAM_CONFIG_DIR/agents`); each is checked as a sibling `roles/<name>.md`,
/// then the built-ins. A `name` containing `/` or ending `.md` is a file path.
pub fn role_prompt(name: &str, search: &[PathBuf]) -> Result<String> {
    if name.contains('/') || name.ends_with(".md") {
        return std::fs::read_to_string(name)
            .with_context(|| format!("reading role prompt {name}"));
    }
    for dir in search {
        let Some(parent) = dir.parent() else { continue };
        let file = parent.join("roles").join(format!("{name}.md"));
        if file.is_file() {
            return std::fs::read_to_string(&file)
                .with_context(|| format!("reading {}", file.display()));
        }
    }
    match builtin_role(name) {
        Some(text) => Ok(text.to_string()),
        None => bail!("no role prompt `{name}`"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_resolve() {
        let w = load("worker", &[]).unwrap();
        assert_eq!(w.cli, "claude");
        assert_eq!(w.model.as_deref(), Some("sonnet"));
        assert_eq!(w.profile.as_deref(), Some("edit"));
        assert_eq!(
            load("orchestrator", &[]).unwrap().model.as_deref(),
            Some("haiku")
        );
        assert!(load("nope", &[]).is_err());
        assert!(role_prompt("worker", &[]).unwrap().contains("worker"));
        assert!(role_prompt("nope", &[]).is_err());
    }

    #[test]
    fn role_prompt_reads_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("custom.md");
        std::fs::write(&file, "from file").unwrap();
        // A `/` marks a path, so a same-named built-in never shadows it.
        let path = file.to_string_lossy().to_string();
        assert_eq!(role_prompt(&path, &[]).unwrap(), "from file");
        // A bare `.md` name is a path relative to the cwd; a missing one errors.
        assert!(role_prompt("missing.md", &[]).is_err());
        assert!(role_prompt(&tmp.path().join("none.md").to_string_lossy(), &[]).is_err());
    }

    #[test]
    fn first_search_dir_wins() {
        let tmp = tempfile::tempdir().unwrap();
        let (a, b) = (tmp.path().join("a/agents"), tmp.path().join("b/agents"));
        for (d, cli) in [(&a, "fake"), (&b, "codex")] {
            std::fs::create_dir_all(d).unwrap();
            std::fs::write(
                d.join("worker.toml"),
                format!("name = \"worker\"\ncli = \"{cli}\"\n"),
            )
            .unwrap();
        }
        let search = [a.clone(), b];
        assert_eq!(load("worker", &search).unwrap().cli, "fake");
        std::fs::create_dir_all(tmp.path().join("a/roles")).unwrap();
        std::fs::write(tmp.path().join("a/roles/worker.md"), "custom").unwrap();
        assert_eq!(role_prompt("worker", &search).unwrap(), "custom");
    }
}

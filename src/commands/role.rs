//! `wteam role <name>`: print the role prompt a turn would receive.

use anyhow::Result;

use super::Out;
use crate::cli::{Global, RoleArgs};
use crate::config;
use crate::paths::Paths;
use crate::team::agents;

/// Where `name` resolves: `project` (`.wteam/roles`), `config` (`$WTEAM_CONFIG_DIR/roles`),
/// `file` (a path), else `built-in`. Mirrors the lookup in `agents::role_prompt`.
fn source(name: &str, project: &std::path::Path, config: &std::path::Path) -> &'static str {
    if name.contains('/') || name.ends_with(".md") {
        "file"
    } else if project.join(format!("{name}.md")).is_file() {
        "project"
    } else if config.join(format!("{name}.md")).is_file() {
        "config"
    } else {
        "built-in"
    }
}

/// Role prompts need no effort, so the slug stays empty: only the `.wteam` root matters.
pub fn run(g: &Global, a: &RoleArgs) -> Result<Out> {
    let paths = Paths::new(super::root(g)?, "");
    let config_dir = config::config_dir();
    let search = [paths.project_agents_dir(), config_dir.join("agents")];
    let text = agents::role_prompt(&a.name, &search)?;
    let src = source(
        &a.name,
        &paths.wteam_dir().join("roles"),
        &config_dir.join("roles"),
    );
    let json = serde_json::json!({ "name": a.name, "source": src, "text": text });
    Ok(Out::new(text, json))
}

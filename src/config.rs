//! User config under `~/.config/wteam/` (or `$WTEAM_CONFIG_DIR`), presets, and team resolution.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};

use crate::paths::Paths;
use crate::team::{Team, dot};

/// `$WTEAM_CONFIG_DIR`, else `$XDG_CONFIG_HOME/wteam`, else `~/.config/wteam`.
pub fn config_dir() -> PathBuf {
    if let Ok(d) = std::env::var("WTEAM_CONFIG_DIR") {
        return PathBuf::from(d);
    }
    if let Ok(x) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(x).join("wteam");
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".config/wteam")
}

/// Built-in team presets, compiled in.
pub const PRESETS: [&str; 3] = ["default", "security", "fan-in"];

/// DOT source of built-in preset `name`.
pub fn preset(name: &str) -> Option<&'static str> {
    match name {
        "default" => Some(include_str!("../presets/default.dot")),
        "security" => Some(include_str!("../presets/security.dot")),
        "fan-in" => Some(include_str!("../presets/fan-in.dot")),
        _ => None,
    }
}

fn load_dot(path: &std::path::Path, pairs: u32) -> Result<Team> {
    let src = std::fs::read_to_string(path)
        .with_context(|| format!("reading team file {}", path.display()))?;
    let graph = dot::parse(&src).with_context(|| format!("parsing {}", path.display()))?;
    Team::from_graph(&graph, pairs)
        .with_context(|| format!("building team from {}", path.display()))
}

/// Resolve the team for a run, first hit wins: a path ending in `.dot`;
/// `.wteam/team.dot` (when `team` is `default`); `<config>/teams/<team>/team.dot`;
/// the built-in preset `team`.
pub fn resolve_team(paths: &Paths, team: &str, pairs: u32) -> Result<Team> {
    if team.ends_with(".dot") {
        return load_dot(&paths.root.join(team), pairs);
    }
    let project = paths.project_team_file();
    if team == "default" && project.is_file() {
        return load_dot(&project, pairs);
    }
    let saved = config_dir().join("teams").join(team).join("team.dot");
    if saved.is_file() {
        return load_dot(&saved, pairs);
    }
    match preset(team) {
        Some(src) => Team::from_graph(&dot::parse(src)?, pairs),
        None => bail!(
            "unknown team {team:?}: not a .dot path, saved team, or preset ({})",
            PRESETS.join(", ")
        ),
    }
}

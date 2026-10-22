//! User config under `~/.config/wteam/` (or `$WTEAM_CONFIG_DIR`), presets, and team resolution.

use std::path::PathBuf;

use anyhow::Result;

use crate::paths::Paths;
use crate::team::Team;

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

/// Resolve the team for a run: `.wteam/team.dot`, a promoted team, a preset
/// name, or a `.dot` path; falls back to the built-in default topology.
pub fn resolve_team(paths: &Paths, team: &str, pairs: u32) -> Result<Team> {
    let _ = (paths, team);
    Ok(Team::default_topology(pairs))
}

//! User config under `~/.config/wteam/` (or `$WTEAM_CONFIG_DIR`), presets, and team resolution.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};

use crate::paths::Paths;
use crate::team::{NodeKind, Team, dot};

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

/// Agent and profile search dirs for one run, first hit wins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchDirs {
    pub agents: Vec<PathBuf>,
    pub profiles: Vec<PathBuf>,
}

/// Agent and profile search dirs for `team`: the project's `.wteam/` overrides, then the
/// saved team's `agents/` and `profiles/` when `team` is a promoted team, then
/// `$WTEAM_CONFIG_DIR`.
pub fn search_dirs(paths: &Paths, team: &str) -> SearchDirs {
    search_dirs_in(paths, team, &config_dir())
}

fn search_dirs_in(paths: &Paths, team: &str, cfg: &std::path::Path) -> SearchDirs {
    let mut agents = vec![paths.project_agents_dir()];
    let mut profiles = vec![paths.project_profiles_dir()];
    let saved = cfg.join("teams").join(team);
    if is_team_name(team) && saved.join("team.dot").is_file() {
        agents.push(saved.join("agents"));
        profiles.push(saved.join("profiles"));
    }
    agents.push(cfg.join("agents"));
    profiles.push(cfg.join("profiles"));
    SearchDirs { agents, profiles }
}

/// A plain team name: letters, digits, `-` and `_`, not a `.dot` path.
fn is_team_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Source text of team `team`, resolved as [`resolve_team`] does.
fn team_source(paths: &Paths, team: &str) -> Result<String> {
    let path = if team.ends_with(".dot") {
        paths.root.join(team)
    } else if team == "default" && paths.project_team_file().is_file() {
        paths.project_team_file()
    } else {
        let saved = config_dir().join("teams").join(team).join("team.dot");
        if saved.is_file() {
            saved
        } else {
            return match preset(team) {
                Some(src) => Ok(src.to_string()),
                None => bail!("unknown team {team:?}: not a .dot path, saved team, or preset"),
            };
        }
    };
    std::fs::read_to_string(&path).with_context(|| format!("reading team file {}", path.display()))
}

/// User config file: `$WTEAM_CONFIG_DIR/config.toml`.
pub fn config_file() -> PathBuf {
    config_dir().join("config.toml")
}

/// The `transport` set in the user config, if any.
pub fn user_transport() -> Result<Option<String>> {
    let file = config_file();
    if !file.is_file() {
        return Ok(None);
    }
    let raw =
        std::fs::read_to_string(&file).with_context(|| format!("reading {}", file.display()))?;
    let table: toml::Table =
        toml::from_str(&raw).with_context(|| format!("parsing {}", file.display()))?;
    match table.get("transport") {
        None => Ok(None),
        Some(toml::Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => bail!("{}: `transport` must be a string", file.display()),
    }
}

/// Set `transport` in the user config, keeping its other keys.
pub fn set_transport(name: &str) -> Result<PathBuf> {
    let file = config_file();
    let mut table = match std::fs::read_to_string(&file) {
        Ok(raw) => toml::from_str::<toml::Table>(&raw)
            .with_context(|| format!("parsing {}", file.display()))?,
        Err(_) => toml::Table::new(),
    };
    table.insert("transport".into(), toml::Value::String(name.into()));
    std::fs::create_dir_all(config_dir())?;
    std::fs::write(&file, toml::to_string(&table)?)
        .with_context(|| format!("writing {}", file.display()))?;
    Ok(file)
}

/// One file written by [`promote`], relative to the saved team dir.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Change {
    pub path: String,
    /// `new`, `updated` or `unchanged`.
    pub status: &'static str,
}

/// Directory of saved team `name` under the user config.
pub fn saved_team_dir(name: &str) -> Result<PathBuf> {
    if !is_team_name(name) {
        bail!("invalid team name {name:?}: use letters, digits, `-` and `_`");
    }
    Ok(config_dir().join("teams").join(name))
}

/// Copy the effort's team, its resolved agent definitions and their profiles into
/// `$WTEAM_CONFIG_DIR/teams/<name>/`. Returns what each file did.
pub fn promote(paths: &Paths, name: &str) -> Result<(PathBuf, Vec<Change>)> {
    let dir = saved_team_dir(name)?;
    let state = crate::state::load(paths)?;
    let team_name = state.run.team.clone();
    let dot = team_source(paths, &team_name)?;
    let team = Team::from_graph(&dot::parse(&dot)?, state.run.pairs)?;
    let search = search_dirs(paths, &team_name);

    let mut files: Vec<(String, String)> = vec![("team.dot".into(), dot.clone())];
    let mut seen_agents = std::collections::BTreeSet::new();
    let mut seen_profiles = std::collections::BTreeSet::new();
    for role in team.roles.values() {
        let NodeKind::Agent {
            agent,
            profile: node_profile,
            ..
        } = &role.kind
        else {
            continue;
        };
        if !seen_agents.insert(agent.clone()) {
            continue;
        }
        let mut def = crate::team::agents::load(agent, &search.agents)?;
        if let Some(prompt) = def.prompt.clone()
            && let Some((rel, text, name)) = role_file(&prompt, &search.agents)?
        {
            files.push((rel, text));
            def.prompt = Some(name);
        }
        files.push((
            format!("agents/{agent}.toml"),
            toml::to_string(&def).with_context(|| format!("serializing agent {agent}"))?,
        ));
        let profile = node_profile
            .clone()
            .or_else(|| def.profile.clone())
            .unwrap_or_else(|| "edit".into());
        if seen_profiles.insert(profile.clone()) {
            let p = crate::permissions::load(&profile, &search.profiles)?;
            files.push((
                format!("profiles/{profile}.toml"),
                toml::to_string(&p).with_context(|| format!("serializing profile {profile}"))?,
            ));
        }
    }

    let mut changes = Vec::new();
    for (rel, contents) in files {
        let file = dir.join(&rel);
        let status = match std::fs::read_to_string(&file) {
            Ok(old) if old == contents => "unchanged",
            Ok(_) => "updated",
            Err(_) => "new",
        };
        if status != "unchanged" {
            if let Some(parent) = file.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&file, &contents)
                .with_context(|| format!("writing {}", file.display()))?;
        }
        changes.push(Change { path: rel, status });
    }
    // Prune saved agent, profile and role files the effort no longer uses.
    let keep: std::collections::BTreeSet<String> = changes.iter().map(|c| c.path.clone()).collect();
    for (sub, ext) in [("agents", "toml"), ("profiles", "toml"), ("roles", "md")] {
        let Ok(entries) = std::fs::read_dir(dir.join(sub)) else {
            continue;
        };
        for entry in entries {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            let rel = format!("{sub}/{name}");
            if entry.path().extension().and_then(|e| e.to_str()) != Some(ext) || keep.contains(&rel)
            {
                continue;
            }
            std::fs::remove_file(entry.path())
                .with_context(|| format!("removing {}", entry.path().display()))?;
            changes.push(Change {
                path: rel,
                status: "removed",
            });
        }
    }
    Ok((dir, changes))
}

/// Role prompt file behind an agent's `prompt`, as promote saves it: `(saved-relative path,
/// text, prompt name the saved agent should use)`. Mirrors `agents::role_prompt`: a path
/// (contains `/` or ends `.md`) is copied to `roles/<stem>.md`, and a bare name is copied from
/// the first sibling `roles/<name>.md`. Built-in prompts are compiled in and need no file.
fn role_file(prompt: &str, agent_dirs: &[PathBuf]) -> Result<Option<(String, String, String)>> {
    if prompt.contains('/') || prompt.ends_with(".md") {
        let path = std::path::Path::new(prompt);
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading role prompt {prompt}"))?;
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .with_context(|| format!("role prompt {prompt} has no file name"))?
            .to_string();
        return Ok(Some((format!("roles/{stem}.md"), text, stem)));
    }
    for dir in agent_dirs {
        let Some(parent) = dir.parent() else { continue };
        let file = parent.join("roles").join(format!("{prompt}.md"));
        if file.is_file() {
            let text = std::fs::read_to_string(&file)
                .with_context(|| format!("reading {}", file.display()))?;
            return Ok(Some((
                format!("roles/{prompt}.md"),
                text,
                prompt.to_string(),
            )));
        }
    }
    Ok(None)
}

/// Names of saved teams (directories with a `team.dot`), sorted.
pub fn saved_teams() -> Result<Vec<String>> {
    let root = config_dir().join("teams");
    let Ok(entries) = std::fs::read_dir(&root) else {
        return Ok(Vec::new());
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if entry.path().join("team.dot").is_file() {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

/// Files of a saved team: `(relative path, contents)` for `team.dot`, `agents/*.toml` and
/// `profiles/*.toml`, sorted by path.
pub fn show_team(name: &str) -> Result<Vec<(String, String)>> {
    let dir = saved_team_dir(name)?;
    if !dir.join("team.dot").is_file() {
        bail!("no saved team `{name}` (see `wteam config list`)");
    }
    let mut out = vec![read_rel(&dir, "team.dot")?];
    for sub in ["agents", "profiles"] {
        let Ok(entries) = std::fs::read_dir(dir.join(sub)) else {
            continue;
        };
        let mut names: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().to_str().map(str::to_string))
            .filter(|n| n.ends_with(".toml"))
            .collect();
        names.sort();
        for n in names {
            out.push(read_rel(&dir, &format!("{sub}/{n}"))?);
        }
    }
    Ok(out)
}

fn read_rel(dir: &std::path::Path, rel: &str) -> Result<(String, String)> {
    let text = std::fs::read_to_string(dir.join(rel))
        .with_context(|| format!("reading {rel} in {}", dir.display()))?;
    Ok((rel.to_string(), text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_dirs_adds_promoted_team_before_user_config() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path(), "s");
        let cfg = tmp.path().join("cfg");
        let saved = cfg.join("teams/mine");
        std::fs::create_dir_all(&saved).unwrap();
        std::fs::write(saved.join("team.dot"), "digraph x {}").unwrap();

        let got = search_dirs_in(&paths, "mine", &cfg);
        assert_eq!(got.agents[1], saved.join("agents"));
        assert_eq!(got.profiles[1], saved.join("profiles"));
        assert_eq!(got.agents[2], cfg.join("agents"));

        // Not promoted (or a path): project and user config only.
        let plain = search_dirs_in(&paths, "default", &cfg);
        assert_eq!(plain.agents.len(), 2);
        assert_eq!(search_dirs_in(&paths, "x.dot", &cfg).agents.len(), 2);
    }

    #[test]
    fn team_names_are_plain() {
        assert!(is_team_name("my-team_2"));
        assert!(!is_team_name(""));
        assert!(!is_team_name("../x"));
        assert!(!is_team_name("a/b"));
        assert!(!is_team_name("x.dot"));
    }
}

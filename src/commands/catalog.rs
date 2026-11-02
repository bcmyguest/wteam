//! `wteam teams` / `wteam phases`: what a run or phase can use, and what each is for.
//! Lists only the entry a name resolves to; a project or saved file hides a built-in.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;
use serde_json::json;

use super::Out;
use crate::cli::Global;
use crate::config;
use crate::phase;
use crate::team::{NodeKind, dot};

#[derive(Debug, Serialize)]
struct Entry {
    name: String,
    source: String,
    description: Option<String>,
    /// Agent roles for a team; personas for a phase.
    members: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

pub fn teams(g: &Global) -> Result<Out> {
    let root = super::root(g)?;
    let mut entries: Vec<Entry> = Vec::new();
    let project = root.join(crate::paths::WTEAM_DIR).join("team.dot");
    if project.is_file() {
        entries.push(team_entry(
            "default",
            "project (.wteam/team.dot)",
            read(&project),
        ));
    }
    for name in config::saved_teams()? {
        if entries.iter().any(|e| e.name == name) {
            continue;
        }
        let file = config::config_dir()
            .join("teams")
            .join(&name)
            .join("team.dot");
        entries.push(team_entry(&name, "saved", read(&file)));
    }
    for name in config::PRESETS {
        if entries.iter().any(|e| e.name == name) {
            continue;
        }
        let src = config::preset(name)
            .map(str::to_string)
            .ok_or_else(|| name.to_string());
        entries.push(team_entry(name, "built-in", src));
    }
    Ok(render("teams", entries))
}

pub fn phases(g: &Global) -> Result<Out> {
    let root = super::root(g)?;
    let wteam = root.join(crate::paths::WTEAM_DIR);
    let dirs = [
        (wteam.join("phases"), "project (.wteam/phases)"),
        (config::config_dir().join("phases"), "config dir"),
    ];
    let search = [wteam.join("agents"), config::config_dir().join("agents")];
    let mut entries: Vec<Entry> = Vec::new();
    let mut add = |name: String, source: &str| {
        if entries.iter().any(|e| e.name == name) {
            return;
        }
        let entry = match phase::load_def(&name, &search) {
            Ok(Some(def)) => Entry {
                name,
                source: source.to_string(),
                description: def.description,
                members: def.personas,
                error: None,
            },
            Ok(None) => return,
            Err(e) => Entry {
                name,
                source: source.to_string(),
                description: None,
                members: Vec::new(),
                error: Some(format!("{e:#}")),
            },
        };
        entries.push(entry);
    };
    for (dir, source) in &dirs {
        for name in toml_stems(dir) {
            add(name, source);
        }
    }
    for name in phase::BUILTIN_PHASES {
        add(name.to_string(), "built-in");
    }
    Ok(render("phases", entries))
}

fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))
}

fn team_entry(name: &str, source: &str, src: Result<String, String>) -> Entry {
    let parsed = src.and_then(|s| {
        let graph = dot::parse(&s).map_err(|e| format!("{e:#}"))?;
        let team = crate::team::Team::from_graph(&graph, 1).map_err(|e| format!("{e:#}"))?;
        Ok((graph, team))
    });
    match parsed {
        Ok((graph, team)) => Entry {
            name: name.to_string(),
            source: source.to_string(),
            description: graph.attrs.get("description").cloned(),
            members: team
                .roles
                .values()
                .filter(|r| matches!(r.kind, NodeKind::Agent { .. }))
                .filter(|r| r.name != "planner" && r.name != "orchestrator")
                .map(|r| match &r.label {
                    Some(l) => format!("{} ({l})", r.name),
                    None => r.name.clone(),
                })
                .collect(),
            error: None,
        },
        Err(e) => Entry {
            name: name.to_string(),
            source: source.to_string(),
            description: None,
            members: Vec::new(),
            error: Some(e),
        },
    }
}

fn toml_stems(dir: &Path) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .filter_map(|p: PathBuf| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .collect();
    names.sort();
    names
}

fn render(kind: &str, entries: Vec<Entry>) -> Out {
    let width = entries.iter().map(|e| e.name.len()).max().unwrap_or(0);
    let text = entries
        .iter()
        .map(|e| {
            let what = match (&e.error, &e.description) {
                (Some(err), _) => format!("error: {err}"),
                (None, Some(d)) => d.clone(),
                (None, None) => "(no description)".to_string(),
            };
            format!("{:width$}  [{}]  {what}", e.name, e.source)
        })
        .collect::<Vec<_>>()
        .join("\n");
    Out::new(text, json!({ kind: entries }))
}

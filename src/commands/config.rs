//! `wteam config`.

use anyhow::Result;
use serde_json::json;

use super::{Out, effort};
use crate::cli::{ConfigCommand, Global};
use crate::config;

pub fn run(g: &Global, c: &ConfigCommand) -> Result<Out> {
    match c {
        ConfigCommand::Promote { name } => promote(g, name),
        ConfigCommand::List => list(),
        ConfigCommand::Show { name } => show(name),
    }
}

/// Copy the effort's team into the user config; print each file as new, updated or unchanged.
fn promote(g: &Global, name: &str) -> Result<Out> {
    let paths = effort(g)?;
    let (dir, changes) = config::promote(&paths, name)?;
    let mut text = format!("promoted `{name}` to {}\n", dir.display());
    for c in &changes {
        text.push_str(&format!("  {} {}\n", c.path, c.status));
    }
    let value = json!({ "team": name, "dir": dir, "changes": changes });
    Ok(Out {
        text: text.trim_end().to_string(),
        json: value,
    })
}

fn list() -> Result<Out> {
    let names = config::saved_teams()?;
    let text = if names.is_empty() {
        "no saved teams".to_string()
    } else {
        names.join("\n")
    };
    Ok(Out {
        text,
        json: json!({ "teams": names }),
    })
}

fn show(name: &str) -> Result<Out> {
    let files = config::show_team(name)?;
    let mut text = String::new();
    for (path, contents) in &files {
        text.push_str(&format!("== {path} ==\n{contents}"));
        if !contents.ends_with('\n') {
            text.push('\n');
        }
    }
    let map: serde_json::Map<String, serde_json::Value> = files
        .into_iter()
        .map(|(p, c)| (p, serde_json::Value::String(c)))
        .collect();
    Ok(Out {
        text: text.trim_end().to_string(),
        json: json!({ "team": name, "files": map }),
    })
}

//! `wteam permissions`.

use std::path::PathBuf;

use anyhow::Result;
use serde_json::json;

use super::Out;
use crate::cli::{Global, PermissionsArgs};
use crate::config;
use crate::paths::Paths;
use crate::permissions::{self, BUILTINS, DEFAULT_CLIS};

/// Profile search dirs: project `.wteam/profiles`, then `$WTEAM_CONFIG_DIR/profiles`.
fn search_dirs(g: &Global) -> Result<Vec<PathBuf>> {
    let root = super::root(g)?;
    Ok(vec![
        Paths::new(root, "").project_profiles_dir(),
        config::config_dir().join("profiles"),
    ])
}

pub fn run(g: &Global, a: &PermissionsArgs) -> Result<Out> {
    let search = search_dirs(g)?;
    let names: Vec<String> = if a.profiles.is_empty() {
        BUILTINS.iter().map(|s| s.to_string()).collect()
    } else {
        a.profiles.clone()
    };
    let clis: Vec<&str> = if a.clis.is_empty() {
        DEFAULT_CLIS.to_vec()
    } else {
        a.clis.iter().map(String::as_str).collect()
    };
    let mut text = String::from("✔ exact   ≈ approximated   ✘ not enforced\n");
    let mut profiles = Vec::new();
    for name in &names {
        let profile = permissions::load(name, &search)?;
        text.push_str(&format!("\n{}\n", profile.name));
        text.push_str(&permissions::support_table(&profile, &clis)?);
        let matrix = permissions::support_matrix(&profile, &clis)?;
        let mut rules = Vec::new();
        for (i, (d, rendered, _)) in permissions::rules_of(&profile)?.iter().enumerate() {
            let mut per_cli = serde_json::Map::new();
            for (cli, support) in &matrix {
                if let Some(rs) = support.get(i) {
                    per_cli.insert(cli.clone(), serde_json::to_value(&rs.support)?);
                }
            }
            rules.push(json!({
                "decision": d,
                "rule": rendered,
                "support": per_cli,
            }));
        }
        profiles.push(json!({
            "name": profile.name,
            "bypass": profile.bypass,
            "rules": rules,
        }));
    }
    Ok(Out {
        text,
        json: json!({ "clis": clis, "profiles": profiles }),
    })
}

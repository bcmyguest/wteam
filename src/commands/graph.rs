//! `wteam graph`.

use std::path::Path;

use anyhow::Result;

use super::Out;
use crate::cli::{Global, GraphArgs};
use crate::paths::{self, Paths};
use crate::{config, render, state, work};

/// `true` when no effort (a `.wteam/<slug>/tickets` dir) exists under `root`.
fn no_efforts(root: &Path) -> bool {
    std::fs::read_dir(root.join(paths::WTEAM_DIR))
        .map(|rd| !rd.flatten().any(|e| e.path().join("tickets").is_dir()))
        .unwrap_or(true)
}

pub fn run(g: &Global, a: &GraphArgs) -> Result<Out> {
    let root = super::root(g)?;
    if a.work {
        let p = super::effort(g)?;
        let tickets = work::load_tickets(&p.tickets_dir())?;
        let st = state::load(&p)?;
        let text = render::work_mermaid(&tickets, &st);
        return Ok(Out::new(
            text.clone(),
            serde_json::json!({ "mermaid": text }),
        ));
    }
    // The team graph needs no effort: only when none exists, fall back to the default
    // team. Any other failure (several efforts, no `--slug`) is an error.
    let slug = match paths::resolve_slug(&root, g.slug.as_deref()) {
        Ok(s) => Some(s),
        Err(_) if no_efforts(&root) => None,
        Err(e) => return Err(e),
    };
    let st = match &slug {
        Some(s) => Some(state::load(&Paths::new(root.clone(), s.clone()))?),
        None => None,
    };
    let p = Paths::new(root, slug.unwrap_or_else(|| "main".into()));
    let name = a
        .team
        .clone()
        .or_else(|| st.as_ref().map(|s| s.run.team.clone()))
        .unwrap_or_else(|| "default".into());
    let pairs = st.as_ref().map_or(1, |s| s.run.pairs);
    let team = config::resolve_team(&p, &name, pairs)?;
    Ok(Out::new(
        render::team_mermaid(&team),
        serde_json::to_value(&team)?,
    ))
}

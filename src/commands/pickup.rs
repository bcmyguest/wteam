//! `wteam pickup`.

use anyhow::{Context, Result};

use super::Out;
use crate::cli::{Global, PickupArgs};
use crate::paths::{self, Paths};
use crate::pickup::{self, PickupReport};
use crate::work;

/// Slug for the effort: `--slug`, else the source directory name, else the only effort.
fn slug(g: &Global, root: &std::path::Path, a: &PickupArgs) -> Result<String> {
    if let Some(s) = &g.slug {
        return Ok(s.clone());
    }
    if let Some(from) = &a.from {
        let abs = resolve_from(root, from)?;
        let name = abs
            .file_name()
            .with_context(|| format!("cannot derive a slug from {}", from.display()))?;
        return Ok(name.to_string_lossy().to_string());
    }
    paths::resolve_slug(root, None)
}

/// A relative `--from` resolves against the cwd, else the repo root.
fn resolve_from(root: &std::path::Path, from: &std::path::Path) -> Result<std::path::PathBuf> {
    let abs = std::path::absolute(from)?;
    if abs.exists() || from.is_absolute() {
        return Ok(abs);
    }
    let in_root = root.join(from);
    Ok(if in_root.exists() { in_root } else { abs })
}

/// Human report in baton-pickup style.
fn render_text(p: &Paths, tickets: usize, r: &PickupReport) -> String {
    let problems = if r.problems.is_empty() {
        "none".to_string()
    } else {
        r.problems
            .iter()
            .map(|x| format!("- {x}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "## Picked up\n\n{} ({tickets} tickets)\n\n## Goal\n\n{}\n\n## Work graph\n\n```mermaid\n{}```\n\n## Problems\n\n{problems}\n\nConfirm to start: `wteam run --pairs N`",
        p.effort_dir().display(),
        r.goal,
        r.graph,
    )
}

pub fn run(g: &Global, a: &PickupArgs) -> Result<Out> {
    let root = super::root(g)?;
    let p = Paths::new(root.clone(), slug(g, &root, a)?);
    let from = a
        .from
        .as_deref()
        .map(|f| resolve_from(&root, f))
        .transpose()?;
    let report = pickup::pickup(&p, from.as_deref())?;
    let tickets = work::load_tickets(&p.tickets_dir())?.len();
    Ok(Out::new(
        render_text(&p, tickets, &report),
        serde_json::to_value(&report)?,
    ))
}

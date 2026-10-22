//! `wteam graph`.

use anyhow::Result;

use super::Out;
use crate::cli::{Global, GraphArgs};
use crate::paths::{self, Paths};
use crate::team::{NodeKind, Team};
use crate::{config, render, state, work};

/// Mermaid node id: alphanumerics and `_` only.
fn mm_id(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Quote text for a Mermaid label.
fn mm_text(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "#quot;"))
}

/// Mermaid flowchart of a team: `[ ]` agent, `[/ /]` command, `{{ }}` human.
pub fn team_mermaid(team: &Team) -> String {
    let mut out = String::from("flowchart LR\n");
    for role in team.roles.values() {
        let name = role.label.as_deref().unwrap_or(&role.name);
        let id = mm_id(&role.name);
        let line = match &role.kind {
            NodeKind::Agent { count, .. } if *count > 1 => {
                format!("{id}[{}]", mm_text(&format!("{name} x{count}")))
            }
            NodeKind::Agent { .. } => format!("{id}[{}]", mm_text(name)),
            NodeKind::Command { .. } => format!("{id}[/{}/]", mm_text(name)),
            NodeKind::Human => format!("{id}{{{{{}}}}}", mm_text(name)),
        };
        out.push_str(&format!("  {line}\n"));
    }
    for l in &team.links {
        let on = l.on.map_or("any", |e| e.name());
        let kind = l.kind.name();
        let text = match &l.label {
            Some(label) => format!("{label} {on}/{kind}"),
            None => format!("{on}/{kind}"),
        };
        out.push_str(&format!(
            "  {} -->|{}| {}\n",
            mm_id(&l.from),
            mm_text(&text),
            mm_id(&l.to)
        ));
    }
    out
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
    // The team graph needs no effort: fall back to the default team.
    let slug = paths::resolve_slug(&root, g.slug.as_deref()).ok();
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
    Ok(Out::new(team_mermaid(&team), serde_json::to_value(&team)?))
}

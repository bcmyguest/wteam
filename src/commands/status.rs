//! `wteam status` and `wteam ready`.

use anyhow::Result;
use serde_json::json;

use super::Out;
use crate::cli::Global;
use crate::state;
use crate::work;

pub fn status(g: &Global) -> Result<Out> {
    let p = super::effort(g)?;
    // Loading the context heals dead turn processes before the state is read.
    crate::engine::Ctx::load(p.clone())?;
    let tickets = work::load_tickets(&p.tickets_dir())?;
    let st = state::load(&p)?;
    let dirty = crate::worktree::dirty_pairs(&p, st.run.pairs);

    let mut text = String::new();
    let mut rows = Vec::new();
    for t in &tickets {
        let ts = st.tickets.get(&t.id).cloned().unwrap_or_default();
        let name = ts.status.as_str();
        text.push_str(&format!("{}  {}  [{}]", t.id, t.title, name));
        if let Some(a) = &ts.assignee {
            text.push_str(&format!("  {a}"));
        }
        text.push('\n');
        rows.push(json!({
            "id": t.id,
            "title": t.title,
            "status": name,
            "assignee": ts.assignee,
            "pair": ts.pair,
            "rounds": ts.rounds,
            "blocked_by": t.blocked_by,
            "blocked_reason": ts.blocked_reason,
        }));
    }
    if tickets.is_empty() {
        text.push_str("no tickets\n");
    }

    if !dirty.is_empty() {
        let list: Vec<String> = dirty.iter().map(|n| format!("pair {n}")).collect();
        text.push_str(&format!(
            "dirty worktrees (uncommitted work): {}\n",
            list.join(", ")
        ));
    }

    let mut agents = Vec::new();
    if !st.agents.is_empty() {
        text.push_str("\nagents:\n");
    }
    for (name, a) in &st.agents {
        let run = if a.busy { "busy" } else { "idle" };
        text.push_str(&format!(
            "  {name}  {run}  turns={}  tokens={}\n",
            a.turns, a.tokens_total
        ));
        agents.push(json!({
            "name": name,
            "role": a.role,
            "busy": a.busy,
            "turns": a.turns,
            "context_tokens": a.context_tokens,
            "tokens_total": a.tokens_total,
        }));
    }
    let tokens_total: u64 = st.agents.values().map(|a| a.tokens_total).sum();
    if !st.agents.is_empty() {
        text.push_str(&format!("  total tokens={tokens_total}\n"));
    }

    let events = st.pending_events();
    if !events.is_empty() {
        text.push_str("\npending events:\n");
    }
    for e in &events {
        text.push_str(&format!(
            "  #{} {:?} {} {}\n",
            e.seq,
            e.kind,
            e.ticket.as_deref().unwrap_or("-"),
            e.text
        ));
    }

    Ok(Out::new(
        text.trim_end().to_string(),
        json!({
            "tickets": rows,
            "agents": agents,
            "tokens_total": tokens_total,
            "events": events,
            "dirty_worktrees": dirty,
        }),
    ))
}

pub fn ready(g: &Global) -> Result<Out> {
    let p = super::effort(g)?;
    let tickets = work::load_tickets(&p.tickets_dir())?;
    let st = state::load(&p)?;
    let ready = work::ready(&tickets, &st);
    let dirty = crate::worktree::dirty_pairs(&p, st.run.pairs);
    let dispatchable = work::dispatchable(&tickets, &st, &dirty);

    let text = if ready.is_empty() {
        "nothing ready".to_string()
    } else {
        ready
            .iter()
            .map(|t| format!("{}  {}", t.id, t.title))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let json = ready
        .iter()
        .map(|t| {
            json!({
                "id": t.id,
                "title": t.title,
                "files": t.files,
                "dispatchable": dispatchable.contains(&t.id),
            })
        })
        .collect::<Vec<_>>();
    Ok(Out::new(text, json!(json)))
}

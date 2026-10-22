//! Ticket lifecycle: dispatch, submit, verdict, block, approve.

use anyhow::{Context, Result, bail};
use serde_json::json;

use super::Out;
use crate::cli::{
    BlockArgs, DispatchArgs, Global, RequeueArgs, SubmitArgs, TicketArg, VerdictArgs, VerdictValue,
};
use crate::engine::{self, Ctx};
use crate::state::{TicketStatus, Verdict, VerdictKind};

pub fn dispatch(g: &Global, a: &DispatchArgs) -> Result<Out> {
    let ctx = Ctx::load(super::effort(g)?)?;
    let agent = engine::dispatch(&ctx, &a.ticket, a.pair)?;
    Ok(Out::new(
        format!("dispatched {} to {agent}", a.ticket),
        json!({ "ticket": a.ticket, "agent": agent }),
    ))
}

pub fn submit(g: &Global, a: &SubmitArgs) -> Result<Out> {
    let ctx = Ctx::load(super::effort(g)?)?;
    let agent = super::caller(g, "human");
    engine::submit(&ctx, &agent, &a.ticket, &a.summary)?;
    Ok(Out::new(
        format!("submitted {} for review", a.ticket),
        json!({ "ticket": a.ticket, "from": agent }),
    ))
}

pub fn verdict(g: &Global, a: &VerdictArgs) -> Result<Out> {
    let ctx = Ctx::load(super::effort(g)?)?;
    let agent = super::caller(g, "human");
    let kind = match a.verdict {
        VerdictValue::Pass => VerdictKind::Pass,
        VerdictValue::Fail => VerdictKind::Fail,
    };
    let follow_ups = a
        .follow_ups
        .iter()
        .map(|f| match f.split_once(':') {
            Some((k, t)) if !k.trim().is_empty() => {
                Ok((k.trim().to_string(), t.trim().to_string()))
            }
            _ => bail!("--follow-up must look like `key:text`, got `{f}`"),
        })
        .collect::<Result<Vec<_>>>()?;
    let v = Verdict {
        kind,
        reviewer: agent.clone(),
        findings: a.findings.clone(),
        notes: a.notes.clone(),
        follow_ups,
    };
    engine::verdict(&ctx, &agent, &a.ticket, v)?;
    Ok(Out::new(
        format!("recorded {:?} for {}", a.verdict, a.ticket).to_lowercase(),
        json!({ "ticket": a.ticket, "verdict": kind, "from": agent }),
    ))
}

pub fn block(g: &Global, a: &BlockArgs) -> Result<Out> {
    let ctx = Ctx::load(super::effort(g)?)?;
    let agent = super::caller(g, "human");
    engine::block(&ctx, &agent, &a.ticket, &a.reason)?;
    Ok(Out::new(
        format!("blocked {}", a.ticket),
        json!({ "ticket": a.ticket, "reason": a.reason }),
    ))
}

/// Move a blocked ticket back to todo: reset rounds and holder, store the note.
pub fn requeue(g: &Global, a: &RequeueArgs) -> Result<Out> {
    let paths = super::effort(g)?;
    let ticket = crate::work::load_tickets(&paths.tickets_dir())?
        .into_iter()
        .find(|t| t.id == a.ticket)
        .with_context(|| format!("no ticket `{}`", a.ticket))?
        .id;
    let state = crate::state::with_state(&paths, |s| {
        let ts = s.ticket_mut(&ticket);
        if ts.status != TicketStatus::Blocked {
            let status = serde_json::to_value(ts.status)?;
            bail!(
                "ticket {ticket} is {}; only blocked tickets can be requeued",
                status.as_str().unwrap_or("not blocked")
            );
        }
        ts.status = TicketStatus::Todo;
        ts.rounds = 0;
        ts.assignee = None;
        ts.pair = None;
        ts.blocked_reason = None;
        ts.note = a.note.clone();
        Ok(ts.clone())
    })?;
    let mut value = serde_json::to_value(&state)?;
    value["ticket"] = json!(ticket);
    Ok(Out::new(format!("requeued {ticket}"), value))
}

pub fn approve(g: &Global, a: &TicketArg) -> Result<Out> {
    let _ = (g, a);
    bail!("not implemented: wteam approve")
}

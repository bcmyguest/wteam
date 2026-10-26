//! Ticket lifecycle: dispatch, submit, verdict, block, approve.

use std::path::Path;

use anyhow::{Context, Result, bail};
use serde_json::json;

use super::Out;
use crate::cli::{
    BlockArgs, DispatchArgs, Global, RequeueArgs, SubmitArgs, TicketArg, TicketsAddArgs,
    TicketsCommand, TicketsListArgs, VerdictArgs, VerdictValue,
};
use crate::engine::{self, Ctx};
use crate::state::{TicketStatus, Verdict, VerdictKind};

pub fn run_subcommand(g: &Global, cmd: &TicketsCommand) -> Result<Out> {
    match cmd {
        TicketsCommand::List(a) => list(g, a),
        TicketsCommand::Add(a) => add(g, a),
    }
}

pub fn add(g: &Global, a: &TicketsAddArgs) -> Result<Out> {
    let p = super::effort(g)?;
    let blocked_by = parse_blocked_by_arg(a.blocked_by.as_deref())?;
    let files = a
        .files
        .as_deref()
        .map(|f| crate::work::split_list(f, ','))
        .unwrap_or_default();
    let verify = a
        .verify
        .as_deref()
        .map(|f| crate::work::split_list(f, ';'))
        .unwrap_or_default();
    let what = a
        .what
        .clone()
        .unwrap_or_else(|| format!("{}.", a.title.trim()));
    let t = crate::work::add_ticket(
        &p.tickets_dir(),
        a.id.as_deref(),
        &a.title,
        &what,
        blocked_by,
        files,
        verify,
    )?;
    let path = t.path.to_string_lossy();
    Ok(Out::new(
        format!("added {} {}", t.id, t.title),
        json!({
            "id": t.id,
            "title": t.title,
            "path": path,
            "blocked_by": t.blocked_by,
            "files": t.files,
            "verify": t.verify,
        }),
    ))
}

fn parse_blocked_by_arg(raw: Option<&str>) -> Result<Vec<String>> {
    let Some(value) = raw else {
        return Ok(Vec::new());
    };
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("none") {
        return Ok(Vec::new());
    }
    let ids = crate::work::parse_blockers(value);
    if ids.is_empty() {
        bail!("could not parse --blocked-by `{value}`");
    }
    Ok(ids)
}

pub fn list(g: &Global, a: &TicketsListArgs) -> Result<Out> {
    let p = super::effort(g)?;
    let tickets = crate::work::load_tickets(&p.tickets_dir())?;
    let st = crate::state::load(&p)?;
    let status_filter = match &a.status {
        Some(raw) => Some(
            TicketStatus::parse_filter(raw)
                .with_context(|| format!("unknown status filter `{raw}`"))?,
        ),
        None => None,
    };

    let mut text = String::new();
    let mut rows = Vec::new();
    for t in &tickets {
        let ts = st.tickets.get(&t.id).cloned().unwrap_or_default();
        if status_filter.is_some_and(|f| ts.status != f) {
            continue;
        }
        let blockers = if t.blocked_by.is_empty() {
            "-".to_string()
        } else {
            t.blocked_by.join(", ")
        };
        text.push_str(&format!(
            "{}  {}  [{}]  blocked_by={}",
            t.id,
            t.title,
            ts.status.as_str(),
            blockers
        ));
        if let Some(a) = &ts.assignee {
            text.push_str(&format!("  assignee={a}"));
        }
        if let Some(pair) = ts.pair {
            text.push_str(&format!("  pair={pair}"));
        }
        text.push('\n');
        rows.push(json!({
            "id": t.id,
            "title": t.title,
            "status": ts.status.as_str(),
            "blocked_by": t.blocked_by,
            "assignee": ts.assignee,
            "pair": ts.pair,
            "rounds": ts.rounds,
            "blocked_reason": ts.blocked_reason,
            "note": ts.note,
            "files": t.files,
            "verify": t.verify,
        }));
    }
    if rows.is_empty() {
        text.push_str("no tickets\n");
    }
    Ok(Out::new(
        text.trim_end().to_string(),
        json!({ "tickets": rows }),
    ))
}

pub fn dispatch(g: &Global, a: &DispatchArgs) -> Result<Out> {
    let ctx = Ctx::load(super::effort(g)?)?;
    if a.fill {
        let fill = engine::fill(&ctx)?;
        if !fill.failed.is_empty() {
            let message = fill
                .failed
                .iter()
                .map(|(ticket, err)| {
                    let state = if fill.blocked.contains(ticket) {
                        "now blocked"
                    } else {
                        "left ready"
                    };
                    format!("dispatch failed for {ticket}, {state}: {err}")
                })
                .collect::<Vec<_>>()
                .join("\n");
            return Err(engine::FillError {
                started: fill.started,
                message,
            }
            .into());
        }
        let text = if fill.started.is_empty() {
            "nothing to dispatch".to_string()
        } else {
            fill.started
                .iter()
                .map(|(ticket, agent)| format!("dispatched {ticket} to {agent}"))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let list: Vec<_> = fill
            .started
            .iter()
            .map(|(ticket, agent)| json!({ "ticket": ticket, "agent": agent }))
            .collect();
        return Ok(Out::new(text, json!({ "dispatched": list })));
    }
    let ticket = a
        .ticket
        .as_deref()
        .context("name a ticket, or pass --fill")?;
    let agent = engine::dispatch(&ctx, ticket, a.pair)?;
    Ok(Out::new(
        format!("dispatched {ticket} to {agent}"),
        json!({ "ticket": ticket, "agent": agent }),
    ))
}

pub fn submit(g: &Global, a: &SubmitArgs) -> Result<Out> {
    let summary = match &a.summary_file {
        Some(p) => read_input(p, "summary")?,
        None => a.summary.clone(),
    };
    let ctx = Ctx::load(super::effort(g)?)?;
    let agent = super::caller(g, "human");
    let dropped = engine::submit(&ctx, &agent, &a.ticket, &summary, &a.extra)?;
    let mut text = String::new();
    if !dropped.is_empty() {
        text.push_str(&format!(
            "not committed: scratch/ and ignored paths are never ticket work: {}\n",
            dropped.join(", ")
        ));
    }
    text.push_str(&format!("submitted {} for review", a.ticket));
    Ok(Out::new(
        text,
        json!({ "ticket": a.ticket, "from": agent, "dropped": dropped }),
    ))
}

pub fn verdict(g: &Global, a: &VerdictArgs) -> Result<Out> {
    let mut sections = match &a.file {
        Some(p) => parse_verdict_file(&read_input(p, "verdict")?)
            .with_context(|| format!("verdict file `{}`", p.display()))?,
        None => VerdictSections::default(),
    };
    sections.findings.extend(a.findings.iter().cloned());
    sections.notes.extend(a.notes.iter().cloned());
    sections.follow_ups.extend(a.follow_ups.iter().cloned());
    let ctx = Ctx::load(super::effort(g)?)?;
    let agent = super::caller(g, "human");
    let kind = match a.verdict {
        VerdictValue::Pass => VerdictKind::Pass,
        VerdictValue::Fail => VerdictKind::Fail,
    };
    let follow_ups = sections
        .follow_ups
        .iter()
        .map(|f| parse_follow_up(f))
        .collect::<Result<Vec<_>>>()?;
    let v = Verdict {
        kind,
        reviewer: agent.clone(),
        findings: sections.findings,
        notes: sections.notes,
        follow_ups,
    };
    engine::verdict(&ctx, &agent, &a.ticket, v)?;
    Ok(Out::new(
        format!("recorded {:?} for {}", a.verdict, a.ticket).to_lowercase(),
        json!({ "ticket": a.ticket, "verdict": kind, "from": agent }),
    ))
}

pub fn block(g: &Global, a: &BlockArgs) -> Result<Out> {
    let reason = match (&a.reason, &a.reason_file) {
        (Some(r), None) => r.clone(),
        (None, Some(p)) => read_input(p, "reason")?.trim().to_string(),
        _ => bail!("give --reason or --reason-file"),
    };
    let ctx = Ctx::load(super::effort(g)?)?;
    let agent = super::caller(g, "human");
    engine::block(&ctx, &agent, &a.ticket, &reason)?;
    Ok(Out::new(
        format!("blocked {}", a.ticket),
        json!({ "ticket": a.ticket, "reason": reason }),
    ))
}

/// Free text from a file, read as-is. Unreadable files fail before any state changes.
fn read_input(path: &Path, what: &str) -> Result<String> {
    std::fs::read_to_string(path)
        .with_context(|| format!("cannot read {what} file `{}`", path.display()))
}

/// Sections of a verdict file, before flags are merged in.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct VerdictSections {
    pub findings: Vec<String>,
    pub notes: Vec<String>,
    pub follow_ups: Vec<String>,
}

/// Parse `FINDINGS:`, `NOTES:` and `FOLLOW-UPS:` sections of `- ` bullets. Blank lines are
/// skipped; anything else outside a bullet under a known header is an error with its line number.
pub fn parse_verdict_file(text: &str) -> Result<VerdictSections> {
    let mut out = VerdictSections::default();
    let mut section: Option<&str> = None;
    for (n, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        match line {
            "FINDINGS:" | "NOTES:" | "FOLLOW-UPS:" => section = Some(line),
            _ => {
                let Some(item) = line.strip_prefix("- ").map(str::trim) else {
                    bail!(
                        "line {}: expected a section header or a `- ` bullet, got `{line}`",
                        n + 1
                    );
                };
                if item.is_empty() {
                    bail!("line {}: empty bullet", n + 1);
                }
                let list = match section {
                    Some("FINDINGS:") => &mut out.findings,
                    Some("NOTES:") => &mut out.notes,
                    Some(_) => &mut out.follow_ups,
                    None => bail!("line {}: bullet before any section header", n + 1),
                };
                list.push(item.to_string());
            }
        }
    }
    Ok(out)
}

/// A follow-up is `key:text`; the key names the issue so repeats count.
fn parse_follow_up(f: &str) -> Result<(String, String)> {
    match f.split_once(':') {
        Some((k, t)) if !k.trim().is_empty() => Ok((k.trim().to_string(), t.trim().to_string())),
        _ => bail!("--follow-up must look like `key:text`, got `{f}`"),
    }
}

/// Move a blocked ticket back to todo: reset rounds and holder, store the note.
pub fn requeue(g: &Global, a: &RequeueArgs) -> Result<Out> {
    let note = match &a.note_file {
        Some(p) => Some(read_input(p, "note")?),
        None => a.note.clone(),
    };
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
        if a.fresh {
            engine::discard_preserved(&paths, s, &ticket)?;
        }
        let ts = s.ticket_mut(&ticket);
        ts.status = TicketStatus::Todo;
        ts.rounds = 0;
        ts.dispatch_failures = 0;
        ts.assignee = None;
        ts.pair = None;
        ts.blocked_reason = None;
        ts.note = note;
        Ok(ts.clone())
    })?;
    let mut value = serde_json::to_value(&state)?;
    value["ticket"] = json!(ticket);
    Ok(Out::new(format!("requeued {ticket}"), value))
}

pub fn approve(g: &Global, a: &TicketArg) -> Result<Out> {
    let ctx = Ctx::load(super::effort(g)?)?;
    engine::approve(&ctx, &a.ticket)?;
    Ok(Out::new(
        format!("approved {}", a.ticket),
        json!({ "ticket": a.ticket }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_three_sections() {
        let text =
            "FINDINGS:\n- a.rs: fix <-> it\n\nNOTES:\n- nit\nFOLLOW-UPS:\n- dup: shared $(x)\n";
        let v = parse_verdict_file(text).unwrap();
        assert_eq!(v.findings, vec!["a.rs: fix <-> it"]);
        assert_eq!(v.notes, vec!["nit"]);
        assert_eq!(v.follow_ups, vec!["dup: shared $(x)"]);
    }

    #[test]
    fn rejects_stray_lines_and_bullets_without_section() {
        assert!(parse_verdict_file("- orphan\n").is_err());
        assert!(parse_verdict_file("FINDINGS:\nplain text\n").is_err());
        assert!(parse_verdict_file("NOTES:\n-\n").is_err());
    }

    #[test]
    fn follow_up_needs_a_key() {
        assert!(parse_follow_up(": text").is_err());
        assert_eq!(
            parse_follow_up("k: t").unwrap(),
            ("k".to_string(), "t".to_string())
        );
    }
}

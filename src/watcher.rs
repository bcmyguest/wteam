//! `wteam run`: start the orchestrator, feed it events, hand it off past the
//! token threshold, and stop with a report when nothing is runnable.
//!
//! The orchestrator never blocks in a CLI call (tool timeouts would kill it).
//! Its turns run here, in the foreground, through the adapter. Between turns the
//! loop waits for workers and reviewers, then resumes it with a digest.

use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

use crate::engine::Ctx;
use crate::mailbox;
use crate::state::{self, Event, State, TicketStatus};
use crate::team::agents::{self, AgentDef};
use crate::team::{NodeKind, parse_instance};
use crate::transport::resume;
use crate::work;

/// Poll interval while waiting for agents.
const POLL: Duration = Duration::from_millis(200);
/// Instance id of the orchestrator.
const ORCHESTRATOR: &str = "orchestrator.1";
/// Sent once per generation, when its context passes the handoff threshold.
const HANDOFF_PROMPT: &str = "Your context is nearly full. Write your handoff baton now, with these sections: Goal, Confirmed facts, Ruled out, Next steps, Constraints, Blockers. Reply with the baton only.";
/// Appended to every kickoff.
const INSTRUCTIONS: &str = "Dispatch every ready ticket to an idle pair with `wteam dispatch <ticket>`, then end your turn. The watcher resumes you with events; never call `wteam wait`.";

#[derive(Debug, Clone)]
pub struct RunOptions {
    /// Seconds to wait for agent turns before giving up.
    pub timeout_secs: u64,
    /// Maximum orchestrator turns (safety cap).
    pub max_turns: u32,
}

/// What the loop does after a poll: resume the orchestrator, or stop.
enum Next {
    Digest(String),
    Stop(String),
}

/// One read of the run state.
struct Snapshot {
    /// Events and mail the orchestrator has not seen, rendered for its prompt.
    digest: Option<String>,
    /// An agent is busy, or a ticket is with a worker or reviewer.
    in_flight: bool,
    /// A ticket awaits a human approval.
    awaiting: bool,
    /// Tickets that are todo with every blocker done.
    ready: usize,
    /// `(used, budget)` when the summed tokens exceed the budget.
    over_budget: Option<(u64, u64)>,
}

/// Drive the orchestrator until the run stops. Returns the stop report text.
pub fn run(ctx: &Ctx, opts: &RunOptions) -> Result<String> {
    let reason = drive(ctx, opts)?;
    let report = crate::report::stop_report(&ctx.paths)?;
    Ok(format!("Run stopped: {reason}.\n\n{report}"))
}

/// The turn loop. Returns why the run stopped.
fn drive(ctx: &Ctx, opts: &RunOptions) -> Result<String> {
    state::with_state(&ctx.paths, |s| {
        if s.orchestrator_generation == 0 {
            s.orchestrator_generation = 1;
        }
        let (role, index) = parse_instance(ORCHESTRATOR);
        let st = s.agents.entry(ORCHESTRATOR.to_string()).or_default();
        if st.role.is_empty() {
            st.role = role;
            st.index = index;
        }
        Ok(())
    })?;
    let mut prompt = kickoff(ctx, None)?;
    let mut turns = 0;
    // Consecutive orchestrator turns that left dispatchable tickets undispatched.
    let mut idle_turns = 0;
    loop {
        if turns >= opts.max_turns {
            return Ok(format!("--max-turns {} reached", opts.max_turns));
        }
        // Handoff turns count toward --max-turns too.
        turns += 1;
        if let Err(e) = orchestrator_turn(ctx, &prompt) {
            return Ok(turn_failed(&e));
        }
        if handoff_due(ctx)? {
            turns += 1;
            let baton = match orchestrator_turn(ctx, HANDOFF_PROMPT) {
                Ok(baton) => baton,
                Err(e) => return Ok(turn_failed(&e)),
            };
            prompt = next_generation(ctx, &baton)?;
            idle_turns = 0;
            continue;
        }
        let undispatched = dispatchable_ready(ctx)?;
        let idle_pairs = idle_pairs(ctx)?;
        // A spent budget means no re-prompt: the await below lets in-flight work finish and stops.
        let budget_spent = over_budget(&state::load(&ctx.paths)?).is_some();
        if !undispatched.is_empty() && idle_pairs > 0 && !budget_spent {
            idle_turns += 1;
            if idle_turns >= 2 {
                return Ok(format!(
                    "orchestrator did not dispatch ready tickets: {}",
                    undispatched.join(", ")
                ));
            }
            prompt = format!(
                "Ready tickets are not dispatched: {}. Idle pairs: {idle_pairs}. Dispatch them now with `wteam dispatch <ticket>`, then end your turn.",
                undispatched.join(", ")
            );
            continue;
        }
        idle_turns = 0;
        match await_next(ctx, opts)? {
            Next::Digest(digest) => prompt = digest,
            Next::Stop(why) => return Ok(why),
        }
    }
}

/// Stop reason for an orchestrator turn that failed.
fn turn_failed(e: &anyhow::Error) -> String {
    format!("orchestrator turn failed: {e:#}")
}

/// One orchestrator turn through the adapter, with the orchestrator marked busy for its
/// duration so `wteam send orchestrator.1` queues mail instead of starting a turn. Returns
/// the reply text.
fn orchestrator_turn(ctx: &Ctx, prompt: &str) -> Result<String> {
    if !resume::begin_foreground(&ctx.paths, ORCHESTRATOR)? {
        bail!("orchestrator is already busy");
    }
    let result = run_orchestrator_turn(ctx, prompt);
    resume::end_foreground(&ctx.paths, ORCHESTRATOR)?;
    result
}

fn run_orchestrator_turn(ctx: &Ctx, prompt: &str) -> Result<String> {
    let s = state::load(&ctx.paths)?;
    let st = s.agents.get(ORCHESTRATOR).cloned().unwrap_or_default();
    // A cleared session means a new generation: send the role prompt again.
    let first = st.session_id.is_none();
    resume::one_turn(
        ctx,
        ORCHESTRATOR,
        first,
        st.session_id,
        st.turns + 1,
        prompt,
        true,
    )
}

/// Ready tickets that could start now: no file overlap with a ticket in progress.
fn dispatchable_ready(ctx: &Ctx) -> Result<Vec<String>> {
    let tickets = work::load_tickets(&ctx.paths.tickets_dir())?;
    let s = state::load(&ctx.paths)?;
    let in_progress: Vec<&work::Ticket> = tickets
        .iter()
        .filter(|t| {
            s.tickets
                .get(&t.id)
                .is_some_and(|ts| matches!(ts.status, TicketStatus::Doing | TicketStatus::Review))
        })
        .collect();
    Ok(work::ready(&tickets, &s)
        .into_iter()
        .filter(|r| !in_progress.iter().any(|p| work::files_overlap(p, r)))
        .map(|r| r.id.clone())
        .collect())
}

/// Pairs with no ticket in progress.
fn idle_pairs(ctx: &Ctx) -> Result<u32> {
    let s = state::load(&ctx.paths)?;
    let busy: std::collections::BTreeSet<u32> = s
        .tickets
        .values()
        .filter(|t| matches!(t.status, TicketStatus::Doing | TicketStatus::Review))
        .filter_map(|t| t.pair)
        .collect();
    Ok(s.run.pairs.saturating_sub(busy.len() as u32))
}

/// True when the orchestrator's last context size passed its handoff threshold.
fn handoff_due(ctx: &Ctx) -> Result<bool> {
    let s = state::load(&ctx.paths)?;
    let Some(tokens) = s.agents.get(ORCHESTRATOR).and_then(|a| a.context_tokens) else {
        return Ok(false);
    };
    Ok(tokens >= handoff_threshold(ctx)?)
}

/// `min(handoff_tokens, 0.75 × context_window)` when the orchestrator's agent sets a window.
fn handoff_threshold(ctx: &Ctx) -> Result<u64> {
    Ok(match orchestrator_def(ctx)?.context_window {
        Some(window) => ctx.team.handoff_tokens.min(window.saturating_mul(3) / 4),
        None => ctx.team.handoff_tokens,
    })
}

/// The agent definition behind the team's orchestrator role.
fn orchestrator_def(ctx: &Ctx) -> Result<AgentDef> {
    let name = match ctx.team.roles.get("orchestrator").map(|r| &r.kind) {
        Some(NodeKind::Agent { agent, .. }) => agent.clone(),
        _ => bail!("team `{}` has no orchestrator agent", ctx.team.name),
    };
    let search = [
        ctx.paths.project_agents_dir(),
        crate::config::config_dir().join("agents"),
    ];
    agents::load(&name, &search)
}

/// Save the baton as this generation's handoff file, start the next generation with a
/// cleared session, and return that generation's kickoff prompt.
fn next_generation(ctx: &Ctx, baton: &str) -> Result<String> {
    let generation = state::load(&ctx.paths)?.orchestrator_generation;
    let dir = ctx.paths.handoffs_dir();
    std::fs::create_dir_all(&dir)?;
    let file = dir.join(format!("orchestrator-{generation}.md"));
    std::fs::write(&file, baton).with_context(|| format!("writing {}", file.display()))?;
    state::with_state(&ctx.paths, |s| {
        s.orchestrator_generation += 1;
        let st = s.agents.entry(ORCHESTRATOR.to_string()).or_default();
        st.session_id = None;
        st.context_tokens = None;
        Ok(())
    })?;
    kickoff(ctx, Some(baton))
}

/// The orchestrator's first prompt of a generation. Without a baton: the goal, pair count
/// and ready tickets. With one: the baton and `wteam status`.
fn kickoff(ctx: &Ctx, baton: Option<&str>) -> Result<String> {
    let ready = wteam_output(ctx, "ready")?;
    Ok(match baton {
        None => {
            let goal = std::fs::read_to_string(ctx.paths.spec_file()).unwrap_or_default();
            let pairs = state::load(&ctx.paths)?.run.pairs;
            format!("Goal:\n{goal}\nPairs: {pairs}\n\nReady:\n{ready}\n\n{INSTRUCTIONS}")
        }
        Some(baton) => {
            let status = wteam_output(ctx, "status")?;
            format!(
                "Handoff from the previous orchestrator generation:\n{baton}\n\nStatus:\n{status}\n\n{INSTRUCTIONS}"
            )
        }
    })
}

/// Stdout of `wteam <cmd>` run from the repo root with this effort selected.
fn wteam_output(ctx: &Ctx, cmd: &str) -> Result<String> {
    let out = Command::new(&ctx.exe)
        .arg(cmd)
        .env("WTEAM_ROOT", &ctx.paths.root)
        .env("WTEAM_SLUG", &ctx.paths.slug)
        .current_dir(&ctx.paths.root)
        .output()
        .with_context(|| format!("running wteam {cmd}"))?;
    if !out.status.success() {
        bail!(
            "wteam {cmd} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
}

/// Wait until the orchestrator has a digest, or the run must stop.
fn await_next(ctx: &Ctx, opts: &RunOptions) -> Result<Next> {
    let deadline = Instant::now() + Duration::from_secs(opts.timeout_secs);
    loop {
        // Re-deliver to agents whose turn process died mid-run, not only at command start.
        crate::engine::heal(ctx)?;
        let snap = snapshot(ctx)?;
        if let Some((used, budget)) = snap.over_budget {
            // No new work: let what is in flight finish, then stop.
            if !snap.in_flight {
                return Ok(Next::Stop(format!(
                    "token budget of {budget} exceeded ({used} tokens used)"
                )));
            }
        } else if let Some(digest) = snap.digest {
            return Ok(Next::Digest(digest));
        } else if !(snap.in_flight || snap.awaiting) {
            return Ok(Next::Stop(if snap.ready > 0 {
                "ready tickets were not dispatched".to_string()
            } else {
                "no ticket is runnable or in progress".to_string()
            }));
        }
        if Instant::now() >= deadline {
            return Ok(Next::Stop(format!(
                "timed out after {} s waiting for agents",
                opts.timeout_secs
            )));
        }
        std::thread::sleep(POLL);
    }
}

/// Read the run once. Takes the orchestrator's pending events and mail unless the budget
/// is spent; a spent budget leaves them unread.
fn snapshot(ctx: &Ctx) -> Result<Snapshot> {
    let tickets = work::load_tickets(&ctx.paths.tickets_dir())?;
    state::with_state(&ctx.paths, |s| {
        let over_budget = over_budget(s);
        let ready = work::ready(&tickets, s).len();
        let in_flight = s
            .agents
            .iter()
            .any(|(id, a)| a.busy && parse_instance(id).0 != "orchestrator")
            || s.tickets
                .values()
                .any(|t| matches!(t.status, TicketStatus::Doing | TicketStatus::Review));
        let awaiting = s
            .tickets
            .values()
            .any(|t| t.status == TicketStatus::AwaitingApproval);
        if over_budget.is_some() {
            return Ok(Snapshot {
                digest: None,
                in_flight,
                awaiting,
                ready,
                over_budget,
            });
        }
        let events = s.pending_events();
        if let Some(last) = events.last() {
            s.events_consumed = last.seq;
        }
        let st = s.agents.entry(ORCHESTRATOR.to_string()).or_default();
        let (mail, end) = mailbox::read_from(&ctx.paths, ORCHESTRATOR, st.mail_delivered)?;
        st.mail_delivered = end;

        let mut parts = Vec::new();
        if !events.is_empty() {
            parts.push(format_events(&events));
        }
        for m in mailbox::parse(&mail) {
            let t = m
                .ticket
                .as_deref()
                .map(|t| format!(" (ticket {t})"))
                .unwrap_or_default();
            parts.push(format!("Message from {}{t}:\n{}", m.from, m.body));
        }
        let digest = (!parts.is_empty())
            .then(|| format!("Events since your last turn:\n\n{}", parts.join("\n\n")));
        Ok(Snapshot {
            digest,
            in_flight,
            awaiting,
            ready,
            over_budget: None,
        })
    })
}

/// `(used, budget)` when the agents' summed `tokens_total` exceeds the run's budget.
fn over_budget(s: &State) -> Option<(u64, u64)> {
    let budget = s.run.budget_tokens?;
    let used: u64 = s.agents.values().map(|a| a.tokens_total).sum();
    (used > budget).then_some((used, budget))
}

/// One line per event: `kind[ ticket]: text`.
pub fn format_events(events: &[Event]) -> String {
    events
        .iter()
        .map(|e| {
            let kind = serde_json::to_value(e.kind)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default();
            let t = e
                .ticket
                .as_deref()
                .map(|t| format!(" {t}"))
                .unwrap_or_default();
            format!("{kind}{t}: {}", e.text)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

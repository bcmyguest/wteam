//! `wteam run`: start the orchestrator, feed it events, hand it off past the
//! token threshold, and stop with a report when nothing is runnable.
//!
//! The orchestrator never blocks in a CLI call (tool timeouts would kill it).
//! Its turns run here, in the foreground, through the adapter. Between turns the
//! loop waits for workers and reviewers, then resumes it with a digest.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

use crate::engine::Ctx;
use crate::mailbox;
use crate::paths::Paths;
use crate::state::{self, Event, EventKind, State, TicketStatus};
use crate::team::agents::{self, AgentDef};
use crate::team::{NodeKind, parse_instance};
use crate::transport::resume;
use crate::work;
use crate::worktree;

/// Poll interval while waiting for agents.
const POLL: Duration = Duration::from_millis(200);
/// Instance id of the orchestrator.
const ORCHESTRATOR: &str = "orchestrator.1";
/// Instance id of the planner that re-plans blocked tickets under `--replan`.
const PLANNER: &str = "planner.1";
/// Sent once per generation, when its context passes the handoff threshold.
const HANDOFF_PROMPT: &str = "Your context is nearly full. Write your handoff baton now, with these sections: Goal, Confirmed facts, Ruled out, Next steps, Constraints, Blockers. Reply with the baton only.";
/// Ends every kickoff and digest.
const FILL_HINT: &str = "Run `wteam dispatch --fill` to keep pairs busy.";
/// Ends every digest under `auto` dispatch: the watcher, not the orchestrator, dispatches.
const AUTO_HINT: &str = "The watcher dispatches ready tickets itself. Act only on the events above, then end your turn.";
/// Appended to every kickoff, before the fill hint.
const INSTRUCTIONS: &str =
    "Then end your turn. The watcher resumes you with events; never call `wteam wait`.";

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
    let mut failed = BTreeMap::new();
    let reason = drive(ctx, opts, &mut failed)?;
    let report = crate::report::stop_report(&ctx.paths)?;
    let mut text = format!("Run stopped: {reason}.\n\n{report}");
    if !failed.is_empty() {
        text.push_str("\n\nDispatch failed (tickets stay ready):");
        for (ticket, err) in &failed {
            text.push_str(&format!("\n- {ticket}: {err}"));
        }
    }
    Ok(text)
}

/// The turn loop. Returns why the run stopped.
fn drive(ctx: &Ctx, opts: &RunOptions, failed: &mut DispatchFailures) -> Result<String> {
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
    if is_auto(ctx)? {
        return drive_auto(ctx, opts, failed);
    }
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
        let doing_before = doing_ids(ctx)?;
        if let Err(e) = orchestrator_turn(ctx, &prompt) {
            return Ok(turn_failed(&e));
        }
        let dispatched = doing_ids(ctx)?.difference(&doing_before).next().is_some();
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
        let (idle_pairs, dirty) = idle_pairs(ctx)?;
        // A spent budget means no re-prompt: the await below lets in-flight work finish and stops.
        let budget_spent = over_budget(&state::load(&ctx.paths)?).is_some();
        if !undispatched.is_empty() && !budget_spent && idle_pairs == 0 && !dirty.is_empty() {
            let list: Vec<String> = dirty.iter().map(|n| format!("pair {n}")).collect();
            return Ok(format!(
                "no usable pair: dirty worktrees on {}",
                list.join(", ")
            ));
        }
        if !undispatched.is_empty() && idle_pairs > 0 && !budget_spent {
            // A turn that started a ticket made progress; only a turn that started none is idle.
            idle_turns = if dispatched { 0 } else { idle_turns + 1 };
            if idle_turns >= 2 {
                return Ok(format!(
                    "orchestrator did not dispatch ready tickets: {}",
                    undispatched.join(", ")
                ));
            }
            prompt = format!(
                "Ready tickets are not dispatched: {}. Idle pairs: {idle_pairs}. Dispatch them now with `wteam dispatch --fill`, then end your turn.",
                undispatched.join(", ")
            );
            continue;
        }
        idle_turns = 0;
        match await_next(ctx, opts, failed)? {
            Next::Digest(digest) => prompt = digest,
            Next::Stop(why) => return Ok(why),
        }
    }
}

/// True when `wteam run --dispatch auto` set the run's dispatch mode.
fn is_auto(ctx: &Ctx) -> Result<bool> {
    Ok(state::load(&ctx.paths)?.run.dispatch == "auto")
}

/// `auto` dispatch. The orchestrator is never prompted for dispatch: `await_next` fills the pairs
/// as they free up. Each wake carries the blocked tickets and human gates; the first wake of a
/// generation also carries the goal. A handoff's baton rides on the next wake.
fn drive_auto(ctx: &Ctx, opts: &RunOptions, failed: &mut DispatchFailures) -> Result<String> {
    let mut turns = 0;
    let mut carried: Option<String> = None;
    loop {
        let digest = match await_next(ctx, opts, failed)? {
            Next::Digest(digest) => digest,
            Next::Stop(why) => return Ok(why),
        };
        if turns >= opts.max_turns {
            return Ok(format!("--max-turns {} reached", opts.max_turns));
        }
        turns += 1;
        let mut prompt = digest;
        if let Some(baton) = carried.take() {
            prompt = format!("{baton}\n\n{prompt}");
        } else if fresh_orchestrator(ctx)? {
            prompt = format!("{}\n\n{prompt}", goal(ctx)?);
        }
        if let Err(e) = orchestrator_turn(ctx, &prompt) {
            return Ok(turn_failed(&e));
        }
        if handoff_due(ctx)? {
            turns += 1;
            let baton = match orchestrator_turn(ctx, HANDOFF_PROMPT) {
                Ok(baton) => baton,
                Err(e) => return Ok(turn_failed(&e)),
            };
            carried = Some(next_generation(ctx, &baton)?);
        }
        if let Some(why) = replan_blocked(ctx)? {
            return Ok(why);
        }
    }
}

/// `--replan`: give each blocked ticket with no re-plan spent one planner turn. The planner
/// requeues the ticket when it can make it succeed; otherwise it stays blocked. Returns a stop
/// reason when a planner turn fails.
fn replan_blocked(ctx: &Ctx) -> Result<Option<String>> {
    let s = state::load(&ctx.paths)?;
    if !s.run.replan || over_budget(&s).is_some() {
        return Ok(None);
    }
    let tickets = work::load_tickets(&ctx.paths.tickets_dir())?;
    let due: Vec<(String, String)> = s
        .tickets
        .iter()
        .filter(|(_, ts)| ts.status == TicketStatus::Blocked && ts.replans == 0)
        .map(|(id, ts)| (id.clone(), ts.blocked_reason.clone().unwrap_or_default()))
        .collect();
    for (id, reason) in due {
        let ticket = tickets.iter().find(|t| t.id == id);
        let title = ticket.map_or_else(String::new, |t| t.title.clone());
        let ticket_path = ticket.map_or_else(PathBuf::new, |t| t.path.clone());
        let pair = if s.run.isolation == "worktree" {
            s.tickets.get(&id).and_then(|ts| ts.pair)
        } else {
            None
        };
        // The budget is spent before the turn, so a failed planner is never retried.
        state::with_state(&ctx.paths, |s| {
            s.ticket_mut(&id).replans += 1;
            Ok(())
        })?;
        let prompt = replan_prompt(
            &ctx.paths,
            &id,
            &title,
            &ticket_path,
            pair,
            &reason,
            ctx.paths.queue_file().exists(),
        );
        if let Err(e) = planner_turn(ctx, &prompt) {
            return Ok(Some(format!("re-plan of {id} failed: {e:#}")));
        }
    }
    Ok(None)
}

/// The planner's prompt for one blocked ticket. Its role prompt rides on the first turn only.
/// It names exact paths and finish commands, so the planner never lists directories.
fn replan_prompt(
    paths: &Paths,
    id: &str,
    title: &str,
    ticket_path: &Path,
    pair: Option<u32>,
    reason: &str,
    queue_exists: bool,
) -> String {
    let slug = &paths.slug;
    let branch = worktree::branch(slug, id);
    let queue = paths.queue_file();
    let queue_note = if queue_exists {
        ""
    } else {
        " (queue.md does not exist yet)"
    };
    let worktree_note = pair.map_or_else(String::new, |n| {
        format!(
            " The preserved work sits in pair {n}'s worktree: {}.",
            worktree::path(paths, n).display()
        )
    });
    let note = paths.effort_dir().join(format!("requeue-{id}.md"));
    format!(
        "Re-plan blocked ticket {id} ({title}). Reason: {reason}. Ticket file: {ticket}. Queue: {queue}{queue_note}. Branch: {branch} (preserved work: `git log {branch}`).{worktree_note} Rewrite the ticket file so it can succeed (narrower scope, corrected Files, notes on preserved work); split it only by adding new ticket files. Run `wteam pickup --slug {slug}` until it lists no problems. Then finish with `wteam requeue {id}`, or, to pass a note into the next dispatch brief, write it to {note} and run `wteam requeue {id} --note-file {note}`.",
        ticket = ticket_path.display(),
        queue = queue.display(),
        note = note.display(),
    )
}

/// One planner turn in the main repo root, marked busy for its duration like the orchestrator.
fn planner_turn(ctx: &Ctx, prompt: &str) -> Result<String> {
    if !resume::begin_foreground(&ctx.paths, PLANNER)? {
        bail!("planner is already busy");
    }
    let result = run_planner_turn(ctx, prompt);
    resume::end_foreground(&ctx.paths, PLANNER)?;
    result
}

fn run_planner_turn(ctx: &Ctx, prompt: &str) -> Result<String> {
    let s = state::load(&ctx.paths)?;
    let st = s.agents.get(PLANNER).cloned().unwrap_or_default();
    let first = st.session_id.is_none();
    resume::one_turn(
        ctx,
        PLANNER,
        first,
        st.session_id,
        st.turns + 1,
        prompt,
        true,
    )
}

/// True when the orchestrator has no session yet: its next turn is the first of a generation.
fn fresh_orchestrator(ctx: &Ctx) -> Result<bool> {
    let s = state::load(&ctx.paths)?;
    Ok(s.agents
        .get(ORCHESTRATOR)
        .is_none_or(|a| a.session_id.is_none()))
}

/// The goal and pair count that open a generation's first prompt.
fn goal(ctx: &Ctx) -> Result<String> {
    let spec = std::fs::read_to_string(ctx.paths.spec_file()).unwrap_or_default();
    let pairs = state::load(&ctx.paths)?.run.pairs;
    Ok(format!("Goal:\n{spec}\nPairs: {pairs}"))
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

/// Ids of tickets whose worker turn is in progress.
fn doing_ids(ctx: &Ctx) -> Result<BTreeSet<String>> {
    let s = state::load(&ctx.paths)?;
    Ok(s.tickets
        .iter()
        .filter(|(_, ts)| ts.status == TicketStatus::Doing)
        .map(|(id, _)| id.clone())
        .collect())
}

/// Usable idle pairs, and the idle pairs that hold uncommitted work. A dirty pair is not idle:
/// dispatch skips it, so it cannot take a ticket.
fn idle_pairs(ctx: &Ctx) -> Result<(u32, Vec<u32>)> {
    let s = state::load(&ctx.paths)?;
    let busy: BTreeSet<u32> = s
        .tickets
        .values()
        .filter(|t| matches!(t.status, TicketStatus::Doing | TicketStatus::Review))
        .filter_map(|t| t.pair)
        .collect();
    let idle: Vec<u32> = (1..=s.run.pairs).filter(|n| !busy.contains(n)).collect();
    let dirty: Vec<u32> = if s.run.isolation == "worktree" {
        idle.iter()
            .copied()
            .filter(|n| worktree::dirty(&ctx.paths, *n))
            .collect()
    } else {
        Vec::new()
    };
    Ok(((idle.len() - dirty.len()) as u32, dirty))
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
    let team = state::load(&ctx.paths)?.run.team;
    let search = crate::config::search_dirs(&ctx.paths, &team);
    agents::load(&name, &search.agents)
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
    let hint = hint(is_auto(ctx)?);
    Ok(match baton {
        None => {
            let goal = goal(ctx)?;
            format!("{goal}\n\nReady:\n{ready}\n\n{INSTRUCTIONS}\n{hint}")
        }
        Some(baton) => {
            let status = wteam_output(ctx, "status")?;
            format!(
                "Handoff from the previous orchestrator generation:\n{baton}\n\nStatus:\n{status}\n\n{INSTRUCTIONS}\n{hint}"
            )
        }
    })
}

/// The line that ends a kickoff or digest, by dispatch mode.
fn hint(auto: bool) -> &'static str {
    if auto { AUTO_HINT } else { FILL_HINT }
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

/// Ticket id to the error of its latest failed auto dispatch. The ticket stays ready and the
/// run goes on; the stop report lists these.
type DispatchFailures = BTreeMap<String, String>;

/// Auto-dispatch what fits. A failed ticket is logged once per distinct error and kept in `failed`.
fn fill_pairs(ctx: &Ctx, failed: &mut DispatchFailures) -> Result<()> {
    let fill = crate::engine::fill(ctx)?;
    for (ticket, _) in &fill.started {
        failed.remove(ticket);
    }
    for (ticket, err) in fill.failed {
        if fill.blocked.contains(&ticket) {
            eprintln!("wteam: ticket {ticket} blocked after failed dispatches: {err}");
            failed.remove(&ticket);
            continue;
        }
        if failed.get(&ticket) != Some(&err) {
            eprintln!("wteam: dispatch of {ticket} failed, left ready: {err}");
        }
        failed.insert(ticket, err);
    }
    Ok(())
}

/// Wait until the orchestrator has a digest, or the run must stop.
fn await_next(ctx: &Ctx, opts: &RunOptions, failed: &mut DispatchFailures) -> Result<Next> {
    let deadline = Instant::now() + Duration::from_secs(opts.timeout_secs);
    let auto = is_auto(ctx)?;
    loop {
        // Re-deliver to agents whose turn process died mid-run, not only at command start.
        crate::engine::heal(ctx)?;
        if auto {
            fill_pairs(ctx, failed)?;
        }
        let snap = snapshot(ctx, auto)?;
        if let Some((used, budget)) = snap.over_budget {
            // No new work: let what is in flight finish, then stop.
            if !snap.in_flight {
                return Ok(Next::Stop(format!(
                    "token budget of {budget} exceeded ({used} tokens used)"
                )));
            }
        } else if let Some(digest) = snap.digest {
            return Ok(Next::Digest(digest));
        } else if !(snap.in_flight || snap.awaiting || !failed.is_empty()) {
            // A ticket with a failed dispatch still gets retried, so the run keeps waiting.
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
fn snapshot(ctx: &Ctx, auto: bool) -> Result<Snapshot> {
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
        // Under `auto` only a block or a human gate wakes the orchestrator; other events are
        // consumed unseen.
        let events: Vec<Event> = events
            .into_iter()
            .filter(|e| !auto || matches!(e.kind, EventKind::Blocked | EventKind::NeedsApproval))
            .collect();
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
        let digest = (!parts.is_empty()).then(|| {
            format!(
                "Events since your last turn:\n\n{}\n\n{}",
                parts.join("\n\n"),
                hint(auto)
            )
        });
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::replan_prompt;
    use crate::paths::Paths;

    fn prompt(pair: Option<u32>, queue_exists: bool) -> String {
        replan_prompt(
            &Paths::new("/repo", "demo"),
            "01",
            "Ticket one",
            Path::new("/repo/.wteam/demo/tickets/01-ticket-one.md"),
            pair,
            "needs spec",
            queue_exists,
        )
    }

    #[test]
    fn replan_prompt_names_the_exact_ticket_file_and_branch() {
        let p = prompt(None, true);
        assert!(
            p.contains("/repo/.wteam/demo/tickets/01-ticket-one.md"),
            "{p}"
        );
        assert!(p.contains("wteam/demo/01"), "{p}");
        assert!(p.contains("/repo/.wteam/demo/queue.md"), "{p}");
        assert!(!p.contains("does not exist"), "{p}");
        assert!(!p.contains("worktrees"), "{p}");
    }

    #[test]
    fn replan_prompt_names_the_pair_worktree_under_isolation() {
        let p = prompt(Some(2), true);
        assert!(p.contains("/repo/.wteam/demo/worktrees/pair2"), "{p}");
    }

    #[test]
    fn replan_prompt_says_when_queue_is_missing() {
        let p = prompt(None, false);
        assert!(p.contains("queue.md does not exist"), "{p}");
    }

    #[test]
    fn replan_prompt_gives_the_exact_finish_commands() {
        let p = prompt(None, true);
        assert!(p.contains("wteam pickup --slug demo"), "{p}");
        assert!(
            p.contains("wteam requeue 01 --note-file /repo/.wteam/demo/requeue-01.md"),
            "{p}"
        );
        assert!(p.contains("wteam requeue 01`"), "{p}");
    }
}

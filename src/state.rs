//! `state.json`: the single mutable record of a run, read and written under a file lock.

use std::collections::BTreeMap;
use std::fs::OpenOptions;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::paths::Paths;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TicketStatus {
    #[default]
    Todo,
    /// A worker holds the ticket.
    Doing,
    /// Submitted; a check or reviewer holds the ticket.
    Review,
    /// Passed review; waiting on a human gate (`wteam approve`).
    AwaitingApproval,
    Done,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerdictKind {
    Pass,
    Fail,
}

/// A reviewer's output for one round. PASS carries no findings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Verdict {
    pub kind: VerdictKind,
    pub reviewer: String,
    #[serde(default)]
    pub findings: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
    /// `(pattern key, text)` pairs appended to `queue.md`.
    #[serde(default)]
    pub follow_ups: Vec<(String, String)>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TicketState {
    pub status: TicketStatus,
    /// Review rounds used (each FAIL or failed check increments).
    #[serde(default)]
    pub rounds: u32,
    /// Agent instance currently holding the ticket, e.g. `worker.2`.
    #[serde(default)]
    pub assignee: Option<String>,
    /// Pair index the ticket was dispatched to.
    #[serde(default)]
    pub pair: Option<u32>,
    #[serde(default)]
    pub commit: Option<String>,
    #[serde(default)]
    pub blocked_reason: Option<String>,
    #[serde(default)]
    pub last_verdict: Option<Verdict>,
    /// Reviewer roles that passed this round. Cleared on FAIL, failed check and dispatch.
    #[serde(default)]
    pub passes: Vec<String>,
    /// Orchestrator note from `wteam requeue`, shown in the next dispatch brief.
    #[serde(default)]
    pub note: Option<String>,
    /// Planner re-plans spent on this ticket under `wteam run --replan` (budget: 1).
    #[serde(default)]
    pub replans: u32,
    /// Changed paths outside `Files:` the worker declared at submit. Committed on PASS.
    /// Replaced by each submit, cleared on dispatch.
    #[serde(default)]
    pub extras: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentState {
    /// Team-graph role node, e.g. `worker`.
    pub role: String,
    /// 1-based instance index.
    pub index: u32,
    #[serde(default)]
    pub session_id: Option<String>,
    /// A turn process is running for this agent.
    #[serde(default)]
    pub busy: bool,
    /// Pid of the running `__turn` process (also its process group id), while busy.
    #[serde(default)]
    pub turn_pid: Option<u32>,
    /// Context size after the last turn, when the CLI reports it.
    #[serde(default)]
    pub context_tokens: Option<u64>,
    #[serde(default)]
    pub turns: u32,
    /// Sum of the per-turn context sizes (`context_tokens`) the CLI reports, over all turns.
    /// It is not billed input or output tokens.
    #[serde(default)]
    pub tokens_total: u64,
    /// When `deliver` started the current turn (unix nanos). Heal uses it to tell a turn
    /// still spawning (no `turn_pid` yet) from one whose spawn died.
    #[serde(default)]
    pub turn_spawned_at: Option<u64>,
    /// Consecutive turns that ended while still holding a ticket without submitting.
    #[serde(default)]
    pub silent_turns: u32,
    /// Byte offset into the agent's mailbox already delivered to it.
    #[serde(default)]
    pub mail_delivered: u64,
    /// Byte offset into the agent's mailbox already shown by `wteam inbox`.
    #[serde(default)]
    pub mail_read: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Passed,
    Blocked,
    NeedsApproval,
    Message,
    /// An agent's turn ended with no further work queued.
    Idle,
}

/// Something the orchestrator should hear about; consumed by `wteam wait`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub seq: u64,
    pub kind: EventKind,
    #[serde(default)]
    pub ticket: Option<String>,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub text: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    pub slug: String,
    #[serde(default)]
    pub tickets: BTreeMap<String, TicketState>,
    /// Keyed by instance id, e.g. `worker.1`.
    #[serde(default)]
    pub agents: BTreeMap<String, AgentState>,
    #[serde(default)]
    pub orchestrator_generation: u32,
    #[serde(default)]
    pub events: Vec<Event>,
    /// Highest event seq consumed by `wteam wait`.
    #[serde(default)]
    pub events_consumed: u64,
    #[serde(default)]
    pub next_seq: u64,
    /// Run settings fixed at `wteam run` / first dispatch.
    #[serde(default)]
    pub run: RunSettings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunSettings {
    pub pairs: u32,
    /// `none` or `worktree`.
    pub isolation: String,
    /// Transport name: `resume` or `herdr`.
    pub transport: String,
    /// Team preset or file the run uses.
    pub team: String,
    /// Stop dispatching once the summed `tokens_total` exceeds this (`wteam run --budget-tokens`).
    #[serde(default)]
    pub budget_tokens: Option<u64>,
    /// A `wteam run` is driving the orchestrator. Set at start, cleared at stop (normal, error,
    /// or `wteam stop`). While set, mail to the orchestrator waits for the watcher's digest.
    #[serde(default)]
    pub active: bool,
    /// Pid of the `wteam run` process that set `active`. A dead pid means the flag is stale
    /// (the run was killed before it could clear the flag).
    #[serde(default)]
    pub watcher_pid: Option<u32>,
    /// `auto` or `model` (`wteam run --dispatch`). A state file without it predates the flag,
    /// whose runs were orchestrator-driven, so it reads as `model`.
    #[serde(default = "model_dispatch")]
    pub dispatch: String,
    /// `wteam run --replan`: under `auto`, the planner rewrites each blocked ticket once.
    #[serde(default)]
    pub replan: bool,
}

fn model_dispatch() -> String {
    "model".into()
}

impl Default for RunSettings {
    fn default() -> Self {
        Self {
            pairs: 1,
            isolation: "none".into(),
            transport: "resume".into(),
            team: "default".into(),
            budget_tokens: None,
            active: false,
            watcher_pid: None,
            dispatch: model_dispatch(),
            replan: false,
        }
    }
}

impl State {
    pub fn new(slug: &str) -> Self {
        Self {
            slug: slug.to_string(),
            ..Default::default()
        }
    }

    /// Append an event for the orchestrator.
    pub fn push_event(
        &mut self,
        kind: EventKind,
        ticket: Option<&str>,
        from: Option<&str>,
        text: impl Into<String>,
    ) {
        self.next_seq += 1;
        self.events.push(Event {
            seq: self.next_seq,
            kind,
            ticket: ticket.map(str::to_string),
            from: from.map(str::to_string),
            text: text.into(),
        });
    }

    /// Events not yet consumed by `wteam wait`.
    pub fn pending_events(&self) -> Vec<Event> {
        self.events
            .iter()
            .filter(|e| e.seq > self.events_consumed)
            .cloned()
            .collect()
    }

    pub fn ticket_mut(&mut self, id: &str) -> &mut TicketState {
        self.tickets.entry(id.to_string()).or_default()
    }
}

/// Current time as unix nanoseconds (0 if the clock is before the epoch).
pub fn now_nanos() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64)
}

/// Read state without locking (for display). Missing file yields a fresh state.
pub fn load(paths: &Paths) -> Result<State> {
    let f = paths.state_file();
    if !f.exists() {
        return Ok(State::new(&paths.slug));
    }
    let raw = std::fs::read_to_string(&f).with_context(|| format!("reading {}", f.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("parsing {}", f.display()))
}

fn save(paths: &Paths, state: &State) -> Result<()> {
    let f = paths.state_file();
    let tmp = f.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(state)?)?;
    std::fs::rename(&tmp, &f)?;
    Ok(())
}

/// Read-modify-write `state.json` under an exclusive lock on `.lock`.
pub fn with_state<T>(paths: &Paths, f: impl FnOnce(&mut State) -> Result<T>) -> Result<T> {
    std::fs::create_dir_all(paths.effort_dir())?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(paths.lock_file())
        .context("opening state lock")?;
    lock.lock().context("locking state")?;
    let mut state = load(paths)?;
    let out = f(&mut state)?;
    save(paths, &state)?;
    lock.unlock().ok();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_state_round_trips_and_numbers_events() {
        let tmp = tempfile::tempdir().unwrap();
        let p = Paths::new(tmp.path(), "s");
        with_state(&p, |s| {
            s.ticket_mut("01").status = TicketStatus::Doing;
            s.push_event(EventKind::Passed, Some("01"), Some("reviewer.1"), "ok");
            s.push_event(EventKind::Blocked, Some("02"), None, "stuck");
            Ok(())
        })
        .unwrap();
        let s = load(&p).unwrap();
        assert_eq!(s.tickets["01"].status, TicketStatus::Doing);
        assert_eq!(
            s.pending_events().iter().map(|e| e.seq).collect::<Vec<_>>(),
            vec![1, 2]
        );
    }

    #[test]
    fn concurrent_writers_do_not_lose_updates() {
        let tmp = tempfile::tempdir().unwrap();
        let p = Paths::new(tmp.path(), "s");
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let p = p.clone();
                std::thread::spawn(move || {
                    for _ in 0..10 {
                        with_state(&p, |s| {
                            s.ticket_mut("01").rounds += 1;
                            Ok(())
                        })
                        .unwrap();
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(load(&p).unwrap().tickets["01"].rounds, 80);
    }
}

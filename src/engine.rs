//! Deterministic routing: agent events on tickets move through the team graph
//! here, never through a model. The orchestrator model only decomposes,
//! dispatches and decides what to do about blocked tickets.

use std::path::PathBuf;

use anyhow::{Result, bail};

use crate::paths::Paths;
use crate::state::Verdict;
use crate::team::Team;

/// Everything a command needs to act on one effort.
#[derive(Debug, Clone)]
pub struct Ctx {
    pub paths: Paths,
    pub team: Team,
    /// Path of the running `wteam` binary (spawned for turns).
    pub exe: PathBuf,
}

impl Ctx {
    /// Load the effort's team (state's `run.team` preset/file, `$pairs` from `run.pairs`).
    pub fn load(paths: Paths) -> Result<Ctx> {
        let state = crate::state::load(&paths)?;
        let team = crate::config::resolve_team(&paths, &state.run.team, state.run.pairs)?;
        let exe = std::env::current_exe()?;
        Ok(Ctx { paths, team, exe })
    }
}

/// Assign `ticket` to a worker (pair `pair`, else the first idle one), brief it, start its turn.
pub fn dispatch(ctx: &Ctx, ticket: &str, pair: Option<u32>) -> Result<String> {
    let _ = (ctx, ticket, pair);
    bail!("not implemented: engine::dispatch")
}

/// A worker finished a round: run command gates, then route to the reviewer.
pub fn submit(ctx: &Ctx, agent: &str, ticket: &str, summary: &str) -> Result<()> {
    let _ = (ctx, agent, ticket, summary);
    bail!("not implemented: engine::submit")
}

/// A reviewer's verdict: PASS commits (or waits on a human gate); FAIL returns to the worker or blocks at the cap.
pub fn verdict(ctx: &Ctx, agent: &str, ticket: &str, verdict: Verdict) -> Result<()> {
    let _ = (ctx, agent, ticket, verdict);
    bail!("not implemented: engine::verdict")
}

/// Mark `ticket` blocked with `reason` and tell the orchestrator.
pub fn block(ctx: &Ctx, agent: &str, ticket: &str, reason: &str) -> Result<()> {
    let _ = (ctx, agent, ticket, reason);
    bail!("not implemented: engine::block")
}

/// Pass a human gate.
pub fn approve(ctx: &Ctx, ticket: &str) -> Result<()> {
    let _ = (ctx, ticket);
    bail!("not implemented: engine::approve")
}

/// Free-form message from one instance to another.
pub fn send(ctx: &Ctx, from: &str, to: &str, body: &str) -> Result<()> {
    let _ = (ctx, from, to, body);
    bail!("not implemented: engine::send")
}

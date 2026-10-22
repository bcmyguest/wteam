//! `wteam run`: start the orchestrator, feed it events, hand it off past the
//! token threshold, and stop with a report when nothing is runnable.

use anyhow::{Result, bail};

use crate::engine::Ctx;

#[derive(Debug, Clone)]
pub struct RunOptions {
    /// Seconds to wait for agent turns before giving up.
    pub timeout_secs: u64,
    /// Maximum orchestrator turns (safety cap).
    pub max_turns: u32,
}

/// Drive the orchestrator until the run stops. Returns the stop report text.
pub fn run(ctx: &Ctx, opts: &RunOptions) -> Result<String> {
    let _ = (ctx, opts);
    bail!("not implemented: watcher::run")
}

//! The default transport: each turn is a headless CLI call that resumes the
//! agent's stored session. `deliver` spawns a detached `wteam __turn <agent>`
//! process; [`run_turns`] is that process's body.

use anyhow::{Result, bail};

use super::Transport;
use crate::engine::Ctx;

pub struct Resume;

impl Transport for Resume {
    fn name(&self) -> &'static str {
        "resume"
    }

    fn deliver(&self, ctx: &Ctx, agent: &str) -> Result<()> {
        let _ = (ctx, agent);
        bail!("not implemented: transport::resume::deliver")
    }
}

/// Run turns for `agent` until its mailbox has no undelivered mail, then mark it idle.
pub fn run_turns(ctx: &Ctx, agent: &str) -> Result<()> {
    let _ = (ctx, agent);
    bail!("not implemented: transport::resume::run_turns")
}

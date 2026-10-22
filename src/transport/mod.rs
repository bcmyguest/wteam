//! Transports deliver mailbox messages to agents and start their turns.
//! Every message is already in the agent's mailbox file before `deliver` runs;
//! a transport only makes the agent read it.

pub mod herdr;
pub mod resume;

use anyhow::{Result, bail};

use crate::engine::Ctx;

pub trait Transport {
    fn name(&self) -> &'static str;
    /// Ensure `agent` gets a turn that consumes its undelivered mail. Non-blocking:
    /// returns once the turn is started (or already running).
    fn deliver(&self, ctx: &Ctx, agent: &str) -> Result<()>;
}

/// The transport for a name (`resume` or `herdr`).
pub fn for_name(name: &str) -> Result<Box<dyn Transport>> {
    Ok(match name {
        "resume" => Box::new(resume::Resume),
        "herdr" => Box::new(herdr::Herdr),
        other => bail!("unknown transport `{other}` (expected resume or herdr)"),
    })
}

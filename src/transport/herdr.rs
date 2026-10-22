//! The herdr transport: agents live in herdr panes; delivery is
//! `herdr agent prompt <name> "<nudge>"`. Requires herdr >= 0.9.5.

use anyhow::{Result, bail};

use super::Transport;
use crate::engine::Ctx;

pub struct Herdr;

impl Transport for Herdr {
    fn name(&self) -> &'static str {
        "herdr"
    }

    fn deliver(&self, ctx: &Ctx, agent: &str) -> Result<()> {
        let _ = (ctx, agent);
        bail!("not implemented: transport::herdr::deliver")
    }
}

//! The codex adapter.

use anyhow::{Result, bail};

use super::{Adapter, CommandSpec, TurnOutput, TurnRequest};

pub struct Codex;

impl Adapter for Codex {
    fn name(&self) -> &'static str {
        "codex"
    }

    fn command(&self, req: &TurnRequest) -> CommandSpec {
        let _ = req;
        CommandSpec::default()
    }

    fn parse_output(&self, stdout: &str) -> Result<TurnOutput> {
        let _ = stdout;
        bail!("not implemented: adapters::codex::parse_output")
    }
}

//! The opencode adapter.

use anyhow::{Result, bail};

use super::{Adapter, CommandSpec, TurnOutput, TurnRequest};

pub struct Opencode;

impl Adapter for Opencode {
    fn name(&self) -> &'static str {
        "opencode"
    }

    fn command(&self, req: &TurnRequest) -> CommandSpec {
        let _ = req;
        CommandSpec::default()
    }

    fn parse_output(&self, stdout: &str) -> Result<TurnOutput> {
        let _ = stdout;
        bail!("not implemented: adapters::opencode::parse_output")
    }
}

//! The claude adapter.

use anyhow::{Result, bail};

use super::{Adapter, CommandSpec, TurnOutput, TurnRequest};

pub struct Claude;

impl Adapter for Claude {
    fn name(&self) -> &'static str {
        "claude"
    }

    fn command(&self, req: &TurnRequest) -> CommandSpec {
        let _ = req;
        CommandSpec::default()
    }

    fn parse_output(&self, stdout: &str) -> Result<TurnOutput> {
        let _ = stdout;
        bail!("not implemented: adapters::claude::parse_output")
    }
}

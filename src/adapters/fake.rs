//! The fake adapter.

use anyhow::{Result, bail};

use super::{Adapter, CommandSpec, TurnOutput, TurnRequest};

pub struct Fake;

impl Adapter for Fake {
    fn name(&self) -> &'static str {
        "fake"
    }

    fn command(&self, req: &TurnRequest) -> CommandSpec {
        let _ = req;
        CommandSpec::default()
    }

    fn parse_output(&self, stdout: &str) -> Result<TurnOutput> {
        let _ = stdout;
        bail!("not implemented: adapters::fake::parse_output")
    }
}

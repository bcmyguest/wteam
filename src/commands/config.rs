//! `wteam config`.

use anyhow::{Result, bail};

use super::Out;
use crate::cli::{Global, ConfigCommand};

pub fn run(g: &Global, c: &ConfigCommand) -> Result<Out> {
    let _ = (g, c);
    bail!("not implemented: wteam config")
}

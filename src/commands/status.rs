//! `wteam status` and `wteam ready`.

use anyhow::{Result, bail};

use super::Out;
use crate::cli::Global;

pub fn status(g: &Global) -> Result<Out> {
    let _ = g;
    bail!("not implemented: wteam status")
}

pub fn ready(g: &Global) -> Result<Out> {
    let _ = g;
    bail!("not implemented: wteam ready")
}

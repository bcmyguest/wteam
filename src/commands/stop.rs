//! `wteam stop` and `wteam restore`.

use anyhow::{Result, bail};

use super::Out;
use crate::cli::Global;

pub fn stop(g: &Global) -> Result<Out> {
    let _ = g;
    bail!("not implemented: wteam stop")
}

pub fn restore(g: &Global) -> Result<Out> {
    let _ = g;
    bail!("not implemented: wteam restore")
}

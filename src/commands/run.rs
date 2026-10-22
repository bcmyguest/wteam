//! `wteam run`.

use anyhow::{Result, bail};

use super::Out;
use crate::cli::{Global, RunArgs};

pub fn run(g: &Global, a: &RunArgs) -> Result<Out> {
    let _ = (g, a);
    bail!("not implemented: wteam run")
}

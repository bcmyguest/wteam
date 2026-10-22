//! `wteam graph`.

use anyhow::{Result, bail};

use super::Out;
use crate::cli::{Global, GraphArgs};

pub fn run(g: &Global, a: &GraphArgs) -> Result<Out> {
    let _ = (g, a);
    bail!("not implemented: wteam graph")
}

//! `wteam setup`.

use anyhow::{Result, bail};

use super::Out;
use crate::cli::{Global, SetupArgs};

pub fn run(g: &Global, a: &SetupArgs) -> Result<Out> {
    let _ = (g, a);
    bail!("not implemented: wteam setup")
}

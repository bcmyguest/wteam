//! `wteam wait`.

use anyhow::{Result, bail};

use super::Out;
use crate::cli::{Global, WaitArgs};

pub fn run(g: &Global, a: &WaitArgs) -> Result<Out> {
    let _ = (g, a);
    bail!("not implemented: wteam wait")
}

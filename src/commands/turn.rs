//! `wteam __turn` (internal).

use anyhow::{Result, bail};

use super::Out;
use crate::cli::{Global, TurnArgs};

pub fn run(g: &Global, a: &TurnArgs) -> Result<Out> {
    let _ = (g, a);
    bail!("not implemented: wteam __turn")
}

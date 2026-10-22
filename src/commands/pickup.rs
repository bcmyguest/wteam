//! `wteam pickup`.

use anyhow::{Result, bail};

use super::Out;
use crate::cli::{Global, PickupArgs};

pub fn run(g: &Global, a: &PickupArgs) -> Result<Out> {
    let _ = (g, a);
    bail!("not implemented: wteam pickup")
}

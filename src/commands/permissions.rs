//! `wteam permissions`.

use anyhow::{Result, bail};

use super::Out;
use crate::cli::{Global, PermissionsArgs};

pub fn run(g: &Global, a: &PermissionsArgs) -> Result<Out> {
    let _ = (g, a);
    bail!("not implemented: wteam permissions")
}

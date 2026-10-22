//! `wteam run`, `stop`, `restore`, `report`.

use anyhow::{Result, bail};

use super::Out;
use crate::cli::{Global, RunArgs};

pub fn run(g: &Global, a: &RunArgs) -> Result<Out> {
    let _ = (g, a);
    bail!("not implemented: wteam run")
}

pub fn stop(g: &Global) -> Result<Out> {
    let _ = g;
    bail!("not implemented: wteam stop")
}

pub fn restore(g: &Global) -> Result<Out> {
    let _ = g;
    bail!("not implemented: wteam restore")
}

pub fn report(g: &Global) -> Result<Out> {
    let _ = g;
    bail!("not implemented: wteam report")
}

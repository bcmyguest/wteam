//! Ticket lifecycle: dispatch, submit, verdict, block, approve.

use anyhow::{Result, bail};

use super::Out;
use crate::cli::{Global, BlockArgs, DispatchArgs, SubmitArgs, TicketArg, VerdictArgs};

pub fn dispatch(g: &Global, a: &DispatchArgs) -> Result<Out> {
    let _ = (g, a);
    bail!("not implemented: wteam dispatch")
}

pub fn submit(g: &Global, a: &SubmitArgs) -> Result<Out> {
    let _ = (g, a);
    bail!("not implemented: wteam submit")
}

pub fn verdict(g: &Global, a: &VerdictArgs) -> Result<Out> {
    let _ = (g, a);
    bail!("not implemented: wteam verdict")
}

pub fn block(g: &Global, a: &BlockArgs) -> Result<Out> {
    let _ = (g, a);
    bail!("not implemented: wteam block")
}

pub fn approve(g: &Global, a: &TicketArg) -> Result<Out> {
    let _ = (g, a);
    bail!("not implemented: wteam approve")
}

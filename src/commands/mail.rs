//! Messaging: send, inbox.

use anyhow::{Result, bail};

use super::Out;
use crate::cli::{Global, InboxArgs, SendArgs};

pub fn send(g: &Global, a: &SendArgs) -> Result<Out> {
    let _ = (g, a);
    bail!("not implemented: wteam send")
}

pub fn inbox(g: &Global, a: &InboxArgs) -> Result<Out> {
    let _ = (g, a);
    bail!("not implemented: wteam inbox")
}

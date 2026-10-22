//! Mailboxes: one append-only markdown file per agent instance under `mail/`.
//! The file is the record; a transport delivers new bytes past
//! `AgentState::mail_delivered`.

use anyhow::{Result, bail};

use crate::paths::Paths;

/// Append a message to `to`'s mailbox. Returns the new file length.
pub fn append(paths: &Paths, to: &str, from: &str, body: &str) -> Result<u64> {
    let _ = (paths, to, from, body);
    bail!("not implemented: mailbox::append")
}

/// Mailbox contents from byte `offset` to the end, and the end offset.
pub fn read_from(paths: &Paths, agent: &str, offset: u64) -> Result<(String, u64)> {
    let _ = (paths, agent, offset);
    bail!("not implemented: mailbox::read_from")
}

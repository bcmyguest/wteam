//! The work graph: tickets in the `/to-tickets` local format, plus readiness.
//!
//! Ticket file shape (`tickets/NN-<slug>.md`):
//!
//! ```markdown
//! # 03: Title
//!
//! **What to build:** prose
//!
//! **Blocked by:** 01, 02        (or "None (can start immediately)")
//!
//! **Status:** ready-for-agent
//!
//! **Files:** src/a.rs, src/b/**   (optional, wteam extension)
//!
//! **Verify:** cargo test -p x     (optional, wteam extension; `;`-separated)
//!
//! - [ ] Acceptance criterion
//! ```

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use serde::Serialize;

use crate::state::State;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ticket {
    /// Two-digit id from the heading, e.g. `03`.
    pub id: String,
    pub title: String,
    pub what: String,
    /// Ids of blocking tickets.
    pub blocked_by: Vec<String>,
    pub status_label: String,
    pub files: Vec<String>,
    pub verify: Vec<String>,
    /// `(checked, text)` acceptance criteria.
    pub criteria: Vec<(bool, String)>,
    pub path: PathBuf,
    /// Full markdown, used verbatim in worker briefs.
    pub body: String,
}

/// Parse one ticket file's markdown. `path` is recorded, not read.
pub fn parse_ticket(markdown: &str, path: &Path) -> Result<Ticket> {
    let _ = (markdown, path);
    bail!("not implemented: work::parse_ticket")
}

/// Load every `*.md` ticket in `dir`, sorted by id.
pub fn load_tickets(dir: &Path) -> Result<Vec<Ticket>> {
    let _ = dir;
    bail!("not implemented: work::load_tickets")
}

/// Problems that make a work graph unrunnable.
pub fn validate(tickets: &[Ticket]) -> Vec<String> {
    let _ = tickets;
    vec!["not implemented: work::validate".into()]
}

/// Tickets whose blockers are all `done` and which are themselves `todo`.
pub fn ready<'a>(tickets: &'a [Ticket], state: &State) -> Vec<&'a Ticket> {
    let _ = (tickets, state);
    Vec::new()
}

/// True when two tickets' `files` globs could touch the same path. Tickets with
/// no `files` overlap everything.
pub fn files_overlap(a: &Ticket, b: &Ticket) -> bool {
    let _ = (a, b);
    true
}

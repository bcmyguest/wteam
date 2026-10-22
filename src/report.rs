//! The stop report: done tickets, blocked tickets with reasons and recommended
//! actions, and follow-up patterns that reached the rule of three.

use anyhow::{Result, bail};

use crate::paths::Paths;

/// Follow-up pattern keys in `queue.md` with their occurrence counts.
pub fn follow_up_counts(paths: &Paths) -> Result<Vec<(String, usize)>> {
    let _ = paths;
    bail!("not implemented: report::follow_up_counts")
}

/// Render the stop report as markdown.
pub fn stop_report(paths: &Paths) -> Result<String> {
    let _ = paths;
    bail!("not implemented: report::stop_report")
}

//! `wteam pickup`: import planning material (`.scratch/<slug>/issues/` from
//! `/to-tickets`, or an existing `.wteam/<slug>/`), then verify it.

use std::path::Path;

use anyhow::{Result, bail};
use serde::Serialize;

use crate::paths::Paths;

#[derive(Debug, Clone, Serialize)]
pub struct PickupReport {
    pub slug: String,
    pub goal: String,
    pub imported: Vec<String>,
    /// Failed checks (empty means ready to run).
    pub problems: Vec<String>,
    /// Mermaid work graph.
    pub graph: String,
}

/// Import from `from` (a `.scratch/<slug>` dir) into `paths`, or verify `paths` in place when `from` is None.
pub fn pickup(paths: &Paths, from: Option<&Path>) -> Result<PickupReport> {
    let _ = (paths, from);
    bail!("not implemented: pickup::pickup")
}

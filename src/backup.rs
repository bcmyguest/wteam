//! Backups for files written outside `.wteam/` (`.codex/rules/…`, `.cursor/cli.json`).
//! Originals (or their absence) are recorded in `backup/manifest.json` before any write.

use std::path::Path;

use anyhow::{Result, bail};

use crate::paths::Paths;

/// Back up `rel` (relative to the repo root) once, then write `contents` to it.
pub fn write_tracked(paths: &Paths, rel: &Path, contents: &str) -> Result<()> {
    let _ = (paths, rel, contents);
    bail!("not implemented: backup::write_tracked")
}

/// Restore every backed-up file and clear the manifest. Returns restored paths.
pub fn restore(paths: &Paths) -> Result<Vec<String>> {
    let _ = paths;
    bail!("not implemented: backup::restore")
}

/// True when a manifest exists (a run wrote files and has not restored them).
pub fn pending(paths: &Paths) -> bool {
    paths.backup_dir().join("manifest.json").exists()
}

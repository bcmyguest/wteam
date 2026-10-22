//! Optional worktree isolation: one git worktree per pair under `worktrees/`,
//! merged back fast-forward after the ticket passes.

use std::path::PathBuf;

use anyhow::{Result, bail};

use crate::paths::Paths;

/// Create (or reuse) pair `pair`'s worktree on branch `wteam/<slug>/<ticket>`.
pub fn prepare(paths: &Paths, pair: u32, ticket: &str) -> Result<PathBuf> {
    let _ = (paths, pair, ticket);
    bail!("not implemented: worktree::prepare")
}

/// Rebase the ticket branch onto the main tree's HEAD and fast-forward it in.
pub fn merge_back(paths: &Paths, pair: u32, ticket: &str) -> Result<String> {
    let _ = (paths, pair, ticket);
    bail!("not implemented: worktree::merge_back")
}

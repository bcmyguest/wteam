//! Optional worktree isolation: one git worktree per pair under `worktrees/`,
//! merged back fast-forward after the ticket passes.

use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::paths::Paths;

/// Git branch a ticket's worktree commits to.
pub fn branch(slug: &str, ticket: &str) -> String {
    format!("wteam/{slug}/{ticket}")
}

/// Directory of pair `pair`'s worktree. It exists only while a ticket holds the pair.
pub fn path(paths: &Paths, pair: u32) -> PathBuf {
    paths.worktrees_dir().join(format!("pair{pair}"))
}

/// Run `git` in `dir`; stdout on success, stderr in the error otherwise.
pub fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .context("running git")?;
    if !out.status.success() {
        bail!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Serialise git work on the main repo across processes (`wteam verdict` runs concurrently).
fn lock(paths: &Paths) -> Result<std::fs::File> {
    std::fs::create_dir_all(paths.effort_dir())?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(paths.effort_dir().join("merge.lock"))
        .context("opening merge lock")?;
    file.lock().context("locking merges")?;
    Ok(file)
}

/// Whether pair `pair`'s worktree has uncommitted changes. A git failure counts as dirty.
pub fn dirty(paths: &Paths, pair: u32) -> bool {
    let dir = path(paths, pair);
    if !dir.is_dir() {
        return false;
    }
    match git(&dir, &["status", "--porcelain"]) {
        Ok(status) => !status.trim().is_empty(),
        Err(_) => true,
    }
}

/// Pairs in `1..=pairs` whose worktree has uncommitted changes.
pub fn dirty_pairs(paths: &Paths, pairs: u32) -> Vec<u32> {
    (1..=pairs).filter(|n| dirty(paths, *n)).collect()
}

/// Create (or reuse) pair `pair`'s worktree on branch `wteam/<slug>/<ticket>`.
/// A new branch starts at the main tree's HEAD. A worktree that still holds another
/// ticket's uncommitted work is refused; a clean one is removed first.
pub fn prepare(paths: &Paths, pair: u32, ticket: &str) -> Result<PathBuf> {
    let _lock = lock(paths)?;
    let root = &paths.root;
    let dir = path(paths, pair);
    let branch = branch(&paths.slug, ticket);
    git(root, &["worktree", "prune"])?;
    if dir.exists() {
        let current = git(&dir, &["branch", "--show-current"])?.trim().to_string();
        if current == branch {
            return Ok(dir);
        }
        if dirty(paths, pair) {
            bail!(
                "pair {pair} worktree holds uncommitted changes from {current}; \
                 clean it before dispatching {ticket}"
            );
        }
        git(
            root,
            &["worktree", "remove", "--force", &dir.to_string_lossy()],
        )?;
    }
    std::fs::create_dir_all(paths.worktrees_dir())?;
    let dir_arg = dir.to_string_lossy();
    let exists = git(
        root,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ],
    )
    .is_ok();
    if exists {
        git(root, &["worktree", "add", &dir_arg, &branch])?;
    } else {
        git(root, &["worktree", "add", "-b", &branch, &dir_arg, "HEAD"])?;
    }
    Ok(dir)
}

/// Rebase the ticket branch onto the main tree's HEAD and fast-forward it in.
/// A rebase conflict aborts the rebase and names the conflicting paths; main is untouched.
/// On success removes the worktree and branch and returns the new main HEAD sha.
pub fn merge_back(paths: &Paths, pair: u32, ticket: &str) -> Result<String> {
    let _lock = lock(paths)?;
    let dir = path(paths, pair);
    let branch = branch(&paths.slug, ticket);
    let base = git(&paths.root, &["rev-parse", "HEAD"])?.trim().to_string();
    if let Err(e) = git(&dir, &["rebase", &base]) {
        let conflicts = git(&dir, &["diff", "--name-only", "--diff-filter=U"]).unwrap_or_default();
        let _ = git(&dir, &["rebase", "--abort"]);
        let paths_list: Vec<&str> = conflicts.lines().collect();
        if paths_list.is_empty() {
            bail!("rebase of {branch} onto main failed: {e:#}");
        }
        bail!(
            "rebase of {branch} onto main conflicts in: {}",
            paths_list.join(", ")
        );
    }
    git(&paths.root, &["merge", "--ff-only", &branch])?;
    let sha = git(&paths.root, &["rev-parse", "HEAD"])?.trim().to_string();
    git(
        &paths.root,
        &["worktree", "remove", "--force", &dir.to_string_lossy()],
    )?;
    git(&paths.root, &["branch", "-D", &branch])?;
    Ok(sha)
}

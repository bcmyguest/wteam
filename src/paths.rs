//! The `.wteam/` directory layout. Every path `wteam` reads or writes is built here.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

/// Name of the state directory at the repo root.
pub const WTEAM_DIR: &str = ".wteam";

/// Locations for one effort (`.wteam/<slug>/`) inside one repo.
#[derive(Debug, Clone)]
pub struct Paths {
    pub root: PathBuf,
    pub slug: String,
}

impl Paths {
    pub fn new(root: impl Into<PathBuf>, slug: impl Into<String>) -> Self {
        Self {
            root: root.into(),
            slug: slug.into(),
        }
    }
    pub fn wteam_dir(&self) -> PathBuf {
        self.root.join(WTEAM_DIR)
    }
    pub fn effort_dir(&self) -> PathBuf {
        self.wteam_dir().join(&self.slug)
    }
    pub fn spec_file(&self) -> PathBuf {
        self.effort_dir().join("spec.md")
    }
    pub fn tickets_dir(&self) -> PathBuf {
        self.effort_dir().join("tickets")
    }
    pub fn state_file(&self) -> PathBuf {
        self.effort_dir().join("state.json")
    }
    pub fn lock_file(&self) -> PathBuf {
        self.effort_dir().join(".lock")
    }
    pub fn mail_dir(&self) -> PathBuf {
        self.effort_dir().join("mail")
    }
    pub fn mail_file(&self, agent: &str) -> PathBuf {
        self.mail_dir().join(format!("{agent}.md"))
    }
    pub fn reviews_dir(&self) -> PathBuf {
        self.effort_dir().join("reviews")
    }
    pub fn queue_file(&self) -> PathBuf {
        self.effort_dir().join("queue.md")
    }
    pub fn handoffs_dir(&self) -> PathBuf {
        self.effort_dir().join("handoffs")
    }
    pub fn logs_dir(&self) -> PathBuf {
        self.effort_dir().join("logs")
    }
    pub fn backup_dir(&self) -> PathBuf {
        self.effort_dir().join("backup")
    }
    pub fn worktrees_dir(&self) -> PathBuf {
        self.effort_dir().join("worktrees")
    }
    /// Project-level overrides: `.wteam/team.dot`, `.wteam/agents/`, `.wteam/profiles/`.
    pub fn project_team_file(&self) -> PathBuf {
        self.wteam_dir().join("team.dot")
    }
    pub fn project_agents_dir(&self) -> PathBuf {
        self.wteam_dir().join("agents")
    }
    pub fn project_profiles_dir(&self) -> PathBuf {
        self.wteam_dir().join("profiles")
    }
    /// Project brief text appended to every worker brief and review request.
    pub fn project_brief_file(&self) -> PathBuf {
        self.wteam_dir().join("brief.md")
    }
    /// Project hook script `.wteam/hooks/<name>`, run with `sh`.
    pub fn project_hook_file(&self, name: &str) -> PathBuf {
        self.wteam_dir().join("hooks").join(name)
    }
}

/// Create `.wteam/` with a self-ignoring `.gitignore` (contents `*`). Idempotent.
pub fn init_wteam_dir(root: &Path) -> Result<PathBuf> {
    let dir = root.join(WTEAM_DIR);
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let ignore = dir.join(".gitignore");
    if !ignore.exists() {
        std::fs::write(&ignore, "*\n")?;
    }
    Ok(dir)
}

/// Create every directory of one effort. Idempotent.
pub fn init_effort(paths: &Paths) -> Result<()> {
    init_wteam_dir(&paths.root)?;
    for d in [
        paths.tickets_dir(),
        paths.mail_dir(),
        paths.reviews_dir(),
        paths.handoffs_dir(),
        paths.logs_dir(),
    ] {
        std::fs::create_dir_all(&d).with_context(|| format!("creating {}", d.display()))?;
    }
    Ok(())
}

/// Resolve the effort slug: the explicit value, else the only effort under `.wteam/`.
pub fn resolve_slug(root: &Path, explicit: Option<&str>) -> Result<String> {
    if let Some(s) = explicit {
        return Ok(s.to_string());
    }
    let dir = root.join(WTEAM_DIR);
    let mut slugs = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if e.path().join("tickets").is_dir() {
                slugs.push(name);
            }
        }
    }
    slugs.sort();
    match slugs.len() {
        1 => Ok(slugs.remove(0)),
        0 => bail!(
            "no effort found under {}; pass --slug or run `wteam pickup`",
            dir.display()
        ),
        _ => bail!(
            "several efforts under {} ({}); pass --slug",
            dir.display(),
            slugs.join(", ")
        ),
    }
}

/// The repo root: `git rev-parse --show-toplevel` from `start`, else `start` itself.
pub fn repo_root(start: &Path) -> PathBuf {
    std::process::Command::new("git")
        .arg("-C")
        .arg(start)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()))
        .unwrap_or_else(|| start.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_writes_self_ignoring_gitignore() {
        let tmp = tempfile::tempdir().unwrap();
        init_wteam_dir(tmp.path()).unwrap();
        let ignore = std::fs::read_to_string(tmp.path().join(".wteam/.gitignore")).unwrap();
        assert_eq!(ignore, "*\n");
    }

    #[test]
    fn resolve_slug_picks_single_effort() {
        let tmp = tempfile::tempdir().unwrap();
        init_effort(&Paths::new(tmp.path(), "alpha")).unwrap();
        assert_eq!(resolve_slug(tmp.path(), None).unwrap(), "alpha");
        init_effort(&Paths::new(tmp.path(), "beta")).unwrap();
        assert!(resolve_slug(tmp.path(), None).is_err());
        assert_eq!(resolve_slug(tmp.path(), Some("beta")).unwrap(), "beta");
    }
}

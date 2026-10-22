//! Backups for files written outside `.wteam/` (`.codex/rules/…`, `.cursor/cli.json`).
//! Originals (or their absence) are recorded in `backup/manifest.json` before any write.

use std::fs::OpenOptions;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::paths::Paths;

/// One tracked file: its repo-relative path and whether it existed before the first write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Entry {
    path: String,
    existed: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Manifest {
    files: Vec<Entry>,
}

fn manifest_file(paths: &Paths) -> PathBuf {
    paths.backup_dir().join("manifest.json")
}

/// Where the original bytes of `rel` are kept.
fn saved_copy(paths: &Paths, rel: &str) -> PathBuf {
    paths.backup_dir().join("files").join(rel)
}

fn load(paths: &Paths) -> Result<Manifest> {
    let f = manifest_file(paths);
    if !f.exists() {
        return Ok(Manifest::default());
    }
    let raw = std::fs::read_to_string(&f).with_context(|| format!("reading {}", f.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("parsing {}", f.display()))
}

fn save(paths: &Paths, m: &Manifest) -> Result<()> {
    let f = manifest_file(paths);
    let tmp = f.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(m)?)?;
    std::fs::rename(&tmp, &f)?;
    Ok(())
}

/// Run `f` holding an exclusive lock on the backup directory.
fn locked<T>(paths: &Paths, f: impl FnOnce() -> Result<T>) -> Result<T> {
    std::fs::create_dir_all(paths.backup_dir())?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(paths.backup_dir().join(".lock"))
        .context("opening backup lock")?;
    lock.lock().context("locking backup")?;
    let out = f();
    lock.unlock().ok();
    out
}

/// `rel` as a normalised repo-relative string; rejects absolute and escaping paths.
fn normalise(rel: &Path) -> Result<String> {
    let mut parts = Vec::new();
    for c in rel.components() {
        match c {
            Component::Normal(p) => parts.push(p.to_string_lossy().into_owned()),
            Component::CurDir => {}
            _ => bail!("tracked path `{}` must stay inside the repo", rel.display()),
        }
    }
    if parts.is_empty() {
        bail!("tracked path is empty");
    }
    Ok(parts.join("/"))
}

/// Back up `rel` (relative to the repo root) once, then write `contents` to it.
pub fn write_tracked(paths: &Paths, rel: &Path, contents: &str) -> Result<()> {
    let key = normalise(rel)?;
    let target = paths.root.join(&key);
    locked(paths, || {
        let mut m = load(paths)?;
        if !m.files.iter().any(|e| e.path == key) {
            let existed = target.exists();
            if existed {
                let copy = saved_copy(paths, &key);
                if let Some(dir) = copy.parent() {
                    std::fs::create_dir_all(dir)?;
                }
                std::fs::copy(&target, &copy)
                    .with_context(|| format!("backing up {}", target.display()))?;
            }
            m.files.push(Entry {
                path: key.clone(),
                existed,
            });
            save(paths, &m)?;
        }
        if let Some(dir) = target.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&target, contents).with_context(|| format!("writing {}", target.display()))
    })
}

/// Remove now-empty directories above `file`, stopping at the repo root.
fn prune_empty_parents(root: &Path, file: &Path) {
    let mut dir = file.parent();
    while let Some(d) = dir {
        if d == root || std::fs::remove_dir(d).is_err() {
            break;
        }
        dir = d.parent();
    }
}

/// Restore every backed-up file and clear the manifest. Returns restored paths.
pub fn restore(paths: &Paths) -> Result<Vec<String>> {
    locked(paths, || {
        let m = load(paths)?;
        let mut restored = Vec::new();
        for e in m.files.iter().rev() {
            let target = paths.root.join(&e.path);
            if e.existed {
                std::fs::copy(saved_copy(paths, &e.path), &target)
                    .with_context(|| format!("restoring {}", target.display()))?;
            } else {
                match std::fs::remove_file(&target) {
                    Ok(()) => prune_empty_parents(&paths.root, &target),
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                    Err(err) => {
                        return Err(err).with_context(|| format!("removing {}", target.display()));
                    }
                }
            }
            restored.push(e.path.clone());
        }
        restored.reverse();
        let _ = std::fs::remove_dir_all(paths.backup_dir().join("files"));
        match std::fs::remove_file(manifest_file(paths)) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.into()),
        }
        Ok(restored)
    })
}

/// True when a manifest exists (a run wrote files and has not restored them).
pub fn pending(paths: &Paths) -> bool {
    manifest_file(paths).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (tempfile::TempDir, Paths) {
        let tmp = tempfile::tempdir().unwrap();
        let p = Paths::new(tmp.path(), "s");
        (tmp, p)
    }

    #[test]
    fn write_tracked_twice_backs_up_the_original_once() {
        let (tmp, p) = setup();
        std::fs::write(tmp.path().join("cfg.json"), "original").unwrap();
        write_tracked(&p, Path::new("cfg.json"), "one").unwrap();
        write_tracked(&p, Path::new("cfg.json"), "two").unwrap();
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("cfg.json")).unwrap(),
            "two"
        );
        let m = load(&p).unwrap();
        assert_eq!(m.files.len(), 1);
        assert_eq!(
            std::fs::read_to_string(saved_copy(&p, "cfg.json")).unwrap(),
            "original"
        );
        assert_eq!(restore(&p).unwrap(), vec!["cfg.json".to_string()]);
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("cfg.json")).unwrap(),
            "original"
        );
    }

    #[test]
    fn restore_deletes_files_that_did_not_exist_and_clears_manifest() {
        let (tmp, p) = setup();
        write_tracked(&p, Path::new(".codex/rules/x.rules"), "r").unwrap();
        assert!(pending(&p));
        restore(&p).unwrap();
        assert!(!tmp.path().join(".codex/rules/x.rules").exists());
        assert!(!tmp.path().join(".codex").exists());
        assert!(!pending(&p));
        assert!(restore(&p).unwrap().is_empty());
    }

    #[test]
    fn rejects_paths_outside_the_repo() {
        let (_tmp, p) = setup();
        assert!(write_tracked(&p, Path::new("../x"), "").is_err());
        assert!(write_tracked(&p, Path::new("/etc/x"), "").is_err());
    }
}

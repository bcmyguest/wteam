//! Shared harness for black-box tests: a temporary git repo, an effort under
//! `.wteam/`, fake-agent role scripts, and a `wteam` command builder.
//!
//! Fake agents: the `fake` adapter runs `sh $WTEAM_FAKE_DIR/<role>.sh`. A script
//! sees `WTEAM_AGENT`, `WTEAM_PROMPT` (this turn's message), `WTEAM_SESSION`
//! (empty on the first turn), `WTEAM_ROOT`, `WTEAM_SLUG`, and may call `wteam`
//! (on `PATH`). It prints `session: <id>` and `tokens: <n>` lines to report a
//! session id and context size; other lines are the turn's text.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::cargo::cargo_bin;

pub struct Repo {
    pub dir: tempfile::TempDir,
    pub slug: String,
}

impl Repo {
    /// A git repo with one commit and an empty effort `slug`.
    pub fn new(slug: &str) -> Repo {
        let dir = tempfile::tempdir().unwrap();
        let r = Repo {
            dir,
            slug: slug.to_string(),
        };
        r.git(&["init", "-q", "-b", "main"]);
        r.git(&["config", "user.email", "test@example.com"]);
        r.git(&["config", "user.name", "test"]);
        r.git(&["config", "commit.gpgsign", "false"]);
        r.write("README.md", "test repo\n");
        r.git(&["add", "-A"]);
        r.git(&["commit", "-q", "-m", "chore: init"]);
        std::fs::create_dir_all(r.fake_dir()).unwrap();
        r.wteam().arg("init").assert().success();
        r
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    pub fn effort(&self) -> PathBuf {
        self.path().join(".wteam").join(&self.slug)
    }

    pub fn fake_dir(&self) -> PathBuf {
        self.path().join(".wteam-fake")
    }

    pub fn write(&self, rel: &str, contents: &str) {
        let p = self.path().join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, contents).unwrap();
    }

    pub fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.path().join(rel)).unwrap_or_default()
    }

    /// Write ticket `NN-name.md` in the to-tickets local format.
    pub fn ticket(&self, id: &str, title: &str, blocked_by: &str, files: &str, verify: &str) {
        let mut body = format!(
            "# {id}: {title}\n\n**What to build:** {title}.\n\n**Blocked by:** {blocked_by}\n\n**Status:** ready-for-agent\n\n"
        );
        if !files.is_empty() {
            body.push_str(&format!("**Files:** {files}\n\n"));
        }
        if !verify.is_empty() {
            body.push_str(&format!("**Verify:** {verify}\n\n"));
        }
        body.push_str("- [ ] it works\n");
        let slug = title.to_lowercase().replace(' ', "-");
        self.write(
            &format!(".wteam/{}/tickets/{id}-{slug}.md", self.slug),
            &body,
        );
    }

    /// Install a fake-agent script for `role`.
    pub fn agent_script(&self, role: &str, script: &str) {
        std::fs::write(self.fake_dir().join(format!("{role}.sh")), script).unwrap();
    }

    /// Make every agent in the effort's team use the fake CLI.
    pub fn use_fake_agents(&self) {
        for role in ["planner", "orchestrator", "worker", "reviewer"] {
            self.write(
                &format!(".wteam/agents/{role}.toml"),
                &format!(
                    "name = \"{role}\"\ncli = \"fake\"\nprompt = \"{role}\"\nprofile = \"edit\"\n"
                ),
            );
        }
    }

    /// `wteam` with root, slug, fake dir and PATH (so scripts can call `wteam`).
    pub fn wteam(&self) -> assert_cmd::Command {
        let mut c = assert_cmd::Command::new(cargo_bin("wteam"));
        c.current_dir(self.path())
            .env_remove("WTEAM_AGENT")
            .env("WTEAM_ROOT", self.path())
            .env("WTEAM_SLUG", &self.slug)
            .env("WTEAM_FAKE_DIR", self.fake_dir())
            .env("WTEAM_CONFIG_DIR", self.path().join(".wteam-config"))
            .env("PATH", path_with_wteam());
        c
    }

    pub fn git(&self, args: &[&str]) -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(self.path())
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).to_string()
    }

    /// `state.json` as JSON.
    pub fn state(&self) -> serde_json::Value {
        serde_json::from_str(&self.read(&format!(".wteam/{}/state.json", self.slug)))
            .unwrap_or_default()
    }

    /// Poll until `pred(state)` holds or `secs` elapse.
    pub fn wait_for(
        &self,
        secs: u64,
        pred: impl Fn(&serde_json::Value) -> bool,
    ) -> serde_json::Value {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(secs);
        loop {
            let s = self.state();
            if pred(&s) {
                return s;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "timed out; state: {s:#}"
            );
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
}

/// `PATH` with the built `wteam` binary's directory first.
pub fn path_with_wteam() -> String {
    let bin = cargo_bin("wteam");
    let dir = bin.parent().unwrap().to_string_lossy().to_string();
    format!("{dir}:{}", std::env::var("PATH").unwrap_or_default())
}

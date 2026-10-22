//! Write/restore/stop through the binary.

mod support;

use std::os::unix::process::CommandExt;

use support::Repo;

/// Track `rel` (with `contents`) the way a turn does: via a state-free helper binary call is not
/// available, so plant the manifest layout through the library.
fn track(r: &Repo, rel: &str, contents: &str) {
    let paths = wteam::paths::Paths::new(r.path(), r.slug.as_str());
    wteam::backup::write_tracked(&paths, std::path::Path::new(rel), contents).unwrap();
}

/// Mark `worker.1` busy in `state.json`, optionally with a recorded turn pid.
fn set_busy(r: &Repo, pid: Option<u32>) {
    let paths = wteam::paths::Paths::new(r.path(), r.slug.as_str());
    wteam::state::with_state(&paths, |s| {
        let a = s.agents.entry("worker.1".into()).or_default();
        a.role = "worker".into();
        a.index = 1;
        a.busy = true;
        a.turn_pid = pid;
        Ok(())
    })
    .unwrap();
}

fn stderr_of(r: &Repo, args: &[&str]) -> String {
    let out = r.wteam().args(args).assert().success();
    String::from_utf8_lossy(&out.get_output().stderr).to_string()
}

const WARNING: &str = "files from an interrupted run are still applied; run `wteam restore`";

#[test]
fn restore_puts_back_originals_and_removes_generated_files() {
    let r = Repo::new("demo");
    r.write(".cursor/cli.json", "original");
    track(&r, ".cursor/cli.json", "changed");
    track(&r, ".codex/rules/wteam-worker.rules", "rules");
    assert_eq!(r.read(".cursor/cli.json"), "changed");

    r.wteam().arg("restore").assert().success();

    assert_eq!(r.read(".cursor/cli.json"), "original");
    assert!(!r.path().join(".codex/rules/wteam-worker.rules").exists());
    assert!(!r.effort().join("backup/manifest.json").exists());
}

#[test]
fn stale_manifest_warns_until_restored() {
    let r = Repo::new("demo");
    assert!(!stderr_of(&r, &["status"]).contains(WARNING));
    track(&r, ".cursor/cli.json", "x");
    assert!(stderr_of(&r, &["status"]).contains(WARNING));
    r.wteam().arg("restore").assert().success();
    assert!(!stderr_of(&r, &["status"]).contains(WARNING));
}

#[test]
fn stop_restores_files_and_marks_busy_agents_stopped() {
    let r = Repo::new("demo");
    track(&r, ".codex/rules/wteam-worker.rules", "rules");
    let mut child = std::process::Command::new("sleep")
        .arg("60")
        .process_group(0)
        .spawn()
        .unwrap();
    let pid = child.id();
    set_busy(&r, Some(pid));

    let out = r.wteam().args(["--json", "stop"]).assert().success();
    let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert_eq!(v["stopped"][0], "worker.1");

    assert!(!r.path().join(".codex/rules/wteam-worker.rules").exists());
    let s = r.state();
    assert_eq!(s["agents"]["worker.1"]["busy"], false);
    assert!(s["agents"]["worker.1"]["turn_pid"].is_null());
    let status = child.wait().unwrap();
    assert!(!status.success(), "turn process group should be killed");
}

#[test]
fn busy_agents_suppress_the_stale_warning() {
    let r = Repo::new("demo");
    track(&r, ".cursor/cli.json", "x");
    set_busy(&r, None);
    assert!(!stderr_of(&r, &["status"]).contains(WARNING));
}

//! `wteam send` and `wteam inbox`.

mod support;

use support::Repo;

fn inbox(r: &Repo, agent: &str, extra: &[&str]) -> String {
    let out = r
        .wteam()
        .env("WTEAM_AGENT", agent)
        .arg("inbox")
        .args(extra)
        .assert()
        .success();
    String::from_utf8(out.get_output().stdout.clone()).unwrap()
}

#[test]
fn send_then_inbox_shows_once_and_all_shows_again() {
    let r = Repo::new("demo");
    r.wteam()
        .args(["send", "worker.1", "please start"])
        .assert()
        .success();
    let first = inbox(&r, "worker.1", &[]);
    assert!(
        first.contains("please start") && first.contains("from human"),
        "{first}"
    );
    assert!(!inbox(&r, "worker.1", &[]).contains("please start"));
    assert!(inbox(&r, "worker.1", &["--all"]).contains("please start"));
}

#[test]
fn send_records_caller_as_sender() {
    let r = Repo::new("demo");
    r.wteam()
        .env("WTEAM_AGENT", "worker.1")
        .args(["send", "orchestrator", "stuck"])
        .assert()
        .success();
    let out = inbox(&r, "orchestrator.1", &[]);
    assert!(
        out.contains("from worker.1") && out.contains("stuck"),
        "{out}"
    );
}

#[test]
fn send_rejects_unknown_targets() {
    let r = Repo::new("demo");
    for to in ["worker.2", "worker.0", "nobody.1", "nobody"] {
        r.wteam().args(["send", to, "hi"]).assert().failure();
    }
}

#[test]
fn inbox_json_shape() {
    let r = Repo::new("demo");
    r.wteam()
        .args(["send", "worker.1", "hello"])
        .assert()
        .success();
    let out = r
        .wteam()
        .env("WTEAM_AGENT", "worker.1")
        .args(["--json", "inbox"])
        .assert()
        .success();
    let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert_eq!(v["agent"], "worker.1");
    assert_eq!(v["messages"][0]["from"], "human");
    assert_eq!(v["messages"][0]["body"], "hello");
    assert!(v["messages"][0]["at"].is_u64());
}

#[test]
fn send_json_shape() {
    let r = Repo::new("demo");
    let out = r
        .wteam()
        .env("WTEAM_AGENT", "worker.1")
        .args(["--json", "send", "orchestrator", "hi"])
        .assert()
        .success();
    let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert_eq!(v["from"], "worker.1");
    assert_eq!(v["to"], "orchestrator");
}

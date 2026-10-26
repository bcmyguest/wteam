//! `wteam tickets` subcommands.

mod support;

use predicates::str::contains;

use support::Repo;

#[test]
fn tickets_add_assigns_ids_and_list_shows_blockers() {
    let r = Repo::new("demo");
    r.wteam()
        .args(["tickets", "add", "Tracer", "--files", "src/core.py"])
        .assert()
        .success()
        .stdout(contains("added 01 Tracer"));
    r.wteam()
        .args([
            "tickets",
            "add",
            "Parser",
            "--blocked-by",
            "01",
            "--files",
            "src/parser.py",
        ])
        .assert()
        .success()
        .stdout(contains("added 02 Parser"));
    r.wteam()
        .args(["tickets", "list"])
        .assert()
        .success()
        .stdout(contains("01  Tracer"))
        .stdout(contains("blocked_by=-"))
        .stdout(contains("02  Parser"))
        .stdout(contains("blocked_by=01"));
}

#[test]
fn tickets_add_rejects_unknown_blocker() {
    let r = Repo::new("demo");
    r.wteam()
        .args(["tickets", "add", "Orphan", "--blocked-by", "99"])
        .assert()
        .failure()
        .stderr(contains("unknown ticket 99"));
}

#[test]
fn tickets_add_json() {
    let r = Repo::new("demo");
    let out = r
        .wteam()
        .args(["--json", "tickets", "add", "Hello"])
        .assert()
        .success();
    let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert_eq!(v["id"], "01");
    assert_eq!(v["title"], "Hello");
}

//! Smoke tests for the binary's surface.

mod support;

use predicates::str::contains;
use support::Repo;

#[test]
fn help_lists_core_commands() {
    let mut c = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin("wteam"));
    c.arg("--help").assert().success().stdout(contains("dispatch")).stdout(contains("verdict"));
}

#[test]
fn init_creates_self_ignoring_dir() {
    let r = Repo::new("demo");
    assert_eq!(r.read(".wteam/.gitignore"), "*\n");
    assert!(r.effort().join("tickets").is_dir());
    assert_eq!(r.git(&["status", "--porcelain"]), "");
}

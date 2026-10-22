//! `wteam graph` and team resolution, driven through the binary.

mod support;

use predicates::str::contains;
use support::Repo;

const CUSTOM: &str = r#"digraph custom {
  coder [agent="worker"]
  lint  [shape=parallelogram, script="cargo clippy"]
  boss  [shape=hexagon]
  coder -> lint [on=submit]
  lint -> boss [on=pass, kind=fan-in]
}
"#;

#[test]
fn graph_prints_default_preset() {
    let r = Repo::new("demo");
    r.wteam()
        .arg("graph")
        .assert()
        .success()
        .stdout(contains("flowchart LR"))
        .stdout(contains("worker[\"worker\"]"))
        .stdout(contains("worker -->|\"submit/pair\"| reviewer"))
        .stdout(contains("reviewer -->|\"pass/fan-in\"| orchestrator"));
}

#[test]
fn project_team_dot_overrides_default() {
    let r = Repo::new("demo");
    r.write(".wteam/team.dot", CUSTOM);
    r.wteam()
        .arg("graph")
        .assert()
        .success()
        .stdout(contains("lint[/\"lint\"/]"))
        .stdout(contains("boss{{\"boss\"}}"))
        .stdout(contains("lint -->|\"pass/fan-in\"| boss"))
        .stdout(contains("planner").not());
}

#[test]
fn named_preset_and_dot_path() {
    let r = Repo::new("demo");
    r.wteam()
        .args(["graph", "--team", "security"])
        .assert()
        .success()
        .stdout(contains("tests[/\"tests\"/]"))
        .stdout(contains("security"));
    r.write("mine.dot", CUSTOM);
    r.wteam()
        .args(["graph", "--team", "mine.dot"])
        .assert()
        .success()
        .stdout(contains("coder"));
}

#[test]
fn json_returns_team_struct() {
    let r = Repo::new("demo");
    let out = r.wteam().args(["--json", "graph"]).output().unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["name"], "default");
    assert_eq!(v["max_visits"], 3);
    assert_eq!(v["roles"]["worker"]["kind"]["type"], "agent");
    assert_eq!(v["links"][1]["on"], "submit");
}

#[test]
fn unknown_team_fails() {
    let r = Repo::new("demo");
    r.wteam()
        .args(["graph", "--team", "nope"])
        .assert()
        .failure()
        .stderr(contains("unknown team"));
}

#[test]
fn bad_dot_reports_line() {
    let r = Repo::new("demo");
    r.write(".wteam/team.dot", "digraph t {\n a [x=]\n}\n");
    r.wteam()
        .arg("graph")
        .assert()
        .failure()
        .stderr(contains("line 2"));
}

use predicates::prelude::PredicateBooleanExt;

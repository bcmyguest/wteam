//! `wteam graph` and team resolution, driven through the binary.

mod support;

use predicates::prelude::PredicateBooleanExt;
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

fn graph_json(r: &Repo, args: &[&str]) -> serde_json::Value {
    let out = r
        .wteam()
        .arg("--json")
        .arg("graph")
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

#[test]
fn security_preset_passes_into_orchestrator_only_from_security() {
    let r = Repo::new("demo");
    let v = graph_json(&r, &["--team", "security"]);
    let into: Vec<&serde_json::Value> = v["links"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|l| l["to"] == "orchestrator" && l["on"] == "pass")
        .collect();
    assert_eq!(into.len(), 1);
    assert_eq!(into[0]["from"], "security");
    assert_eq!(into[0]["kind"], "fan-in");
}

#[test]
fn fan_in_preset_has_one_reviewer_and_pairs_workers() {
    let r = Repo::new("demo");
    r.write(
        &format!(".wteam/{}/state.json", r.slug),
        r#"{"slug": "demo", "run": {"pairs": 3, "isolation": "none", "transport": "resume", "team": "default"}}"#,
    );
    let v = graph_json(&r, &["--team", "fan-in"]);
    assert_eq!(v["roles"]["reviewer"]["kind"]["count"], 1);
    assert_eq!(v["roles"]["worker"]["kind"]["count"], 3);
}

#[test]
fn saved_team_resolves_from_config_dir() {
    let r = Repo::new("demo");
    r.write(".wteam-config/teams/mine/team.dot", CUSTOM);
    r.wteam()
        .args(["graph", "--team", "mine"])
        .assert()
        .success()
        .stdout(contains("coder"))
        .stdout(contains("planner").not());
}

#[test]
fn graph_errors_on_ambiguous_efforts() {
    let r = Repo::new("demo");
    r.wteam()
        .args(["--slug", "other", "init"])
        .assert()
        .success();
    r.wteam()
        .env_remove("WTEAM_SLUG")
        .arg("graph")
        .assert()
        .failure()
        .stderr(contains("several efforts"));
}

#[test]
fn graph_without_any_effort_uses_default_team() {
    let r = Repo::new("demo");
    std::fs::remove_dir_all(r.effort()).unwrap();
    r.wteam()
        .env_remove("WTEAM_SLUG")
        .arg("graph")
        .assert()
        .success()
        .stdout(contains("flowchart LR"))
        .stdout(contains("worker"));
}

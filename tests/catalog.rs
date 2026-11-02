//! `wteam teams` / `wteam phases`: what a run or phase can use, with descriptions.

mod support;

use predicates::str::contains;
use support::Repo;

fn listed(repo: &Repo, cmd: &str) -> Vec<serde_json::Value> {
    let out = repo.wteam().args(["--json", cmd]).assert().success();
    let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    v[cmd].as_array().unwrap().clone()
}

fn find<'a>(entries: &'a [serde_json::Value], name: &str) -> &'a serde_json::Value {
    entries
        .iter()
        .find(|e| e["name"] == name)
        .unwrap_or_else(|| panic!("{name} not listed in {entries:?}"))
}

#[test]
fn built_in_teams_and_phases_say_what_they_are_for() {
    let r = Repo::new("demo");
    let teams = listed(&r, "teams");
    for name in ["default", "security", "fan-in"] {
        let t = find(&teams, name);
        assert_eq!(t["source"], "built-in");
        assert!(t["description"].as_str().unwrap().contains("Use "), "{t}");
    }
    assert_eq!(
        find(&teams, "default")["members"],
        serde_json::json!(["reviewer", "worker"])
    );
    let phases = listed(&r, "phases");
    for name in ["plan-review", "validate"] {
        assert!(
            find(&phases, name)["description"]
                .as_str()
                .unwrap()
                .contains("Use ")
        );
    }
}

#[test]
fn project_and_saved_files_are_listed_and_shadow_built_ins() {
    let r = Repo::new("demo");
    r.write(
        ".wteam/team.dot",
        "digraph mine { graph [description=\"Project team.\"] orchestrator -> worker [on=dispatch] }\n",
    );
    r.write(
        ".wteam-config/teams/bugfix/team.dot",
        "digraph bugfix { graph [description=\"Diagnose, then audit.\"] worker [label=\"diagnose-failure\"] orchestrator -> worker [on=dispatch] }\n",
    );
    r.write(
        ".wteam/phases/validate.toml",
        "description = \"Project validate.\"\npersonas = [\"validate-out\"]\n",
    );
    r.write(
        ".wteam-config/phases/doc-review.toml",
        "description = \"Review a document.\"\npersonas = [\"doc-a\"]\nrequires_input = true\n",
    );
    let teams = listed(&r, "teams");
    assert_eq!(find(&teams, "default")["description"], "Project team.");
    assert_eq!(teams.iter().filter(|t| t["name"] == "default").count(), 1);
    let bugfix = find(&teams, "bugfix");
    assert_eq!(bugfix["source"], "saved");
    assert_eq!(
        bugfix["members"],
        serde_json::json!(["worker (diagnose-failure)"])
    );
    let phases = listed(&r, "phases");
    assert_eq!(
        find(&phases, "validate")["description"],
        "Project validate."
    );
    assert_eq!(phases.iter().filter(|p| p["name"] == "validate").count(), 1);
    assert_eq!(find(&phases, "doc-review")["source"], "config dir");
}

#[test]
fn a_broken_file_is_reported_without_hiding_the_rest() {
    let r = Repo::new("demo");
    r.write(".wteam/phases/bad.toml", "personas = [\"a\"]\ntypo = 1\n");
    r.write(
        ".wteam-config/teams/broken/team.dot",
        "digraph broken { a -> \n",
    );
    r.wteam()
        .args(["phases"])
        .assert()
        .success()
        .stdout(contains("bad"))
        .stdout(contains("error:"))
        .stdout(contains("plan-review"));
    r.wteam()
        .args(["teams"])
        .assert()
        .success()
        .stdout(contains("broken"))
        .stdout(contains("error:"))
        .stdout(contains("fan-in"));
}

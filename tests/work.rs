//! `wteam status` and `wteam ready` through the binary.

mod support;

use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use support::Repo;

fn repo() -> Repo {
    let r = Repo::new("demo");
    r.ticket(
        "01",
        "First thing",
        "None (can start immediately)",
        "src/a.rs",
        "",
    );
    r.ticket(
        "02",
        "Second thing",
        "01",
        "src/b.rs, docs/**",
        "cargo test",
    );
    r.ticket("03", "Third thing", "#1, 2", "", "");
    r
}

fn set_state(r: &Repo, json: serde_json::Value) {
    r.write(&format!(".wteam/{}/state.json", r.slug), &json.to_string());
}

fn json_of(r: &Repo, args: &[&str]) -> serde_json::Value {
    let out = r.wteam().arg("--json").args(args).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

#[test]
fn ready_lists_unblocked_todo_tickets() {
    let r = repo();
    r.wteam()
        .arg("ready")
        .assert()
        .success()
        .stdout(contains("01  First thing"))
        .stdout(contains("Second thing").not());
}

#[test]
fn ready_follows_done_blockers_and_skips_started_tickets() {
    let r = repo();
    set_state(
        &r,
        serde_json::json!({"slug": "demo", "tickets": {
            "01": {"status": "done"}, "02": {"status": "doing"}}}),
    );
    let v = json_of(&r, &["ready"]);
    assert_eq!(v, serde_json::json!([]));
    set_state(
        &r,
        serde_json::json!({"slug": "demo", "tickets": {"01": {"status": "done"}}}),
    );
    let v = json_of(&r, &["ready"]);
    assert_eq!(v[0]["id"], "02");
    assert_eq!(v[0]["title"], "Second thing");
    assert_eq!(v[0]["files"], serde_json::json!(["src/b.rs", "docs/**"]));
    assert_eq!(v.as_array().unwrap().len(), 1);
}

#[test]
fn status_defaults_to_todo_and_shows_ticket_titles() {
    let r = repo();
    r.wteam()
        .arg("status")
        .assert()
        .success()
        .stdout(contains("01  First thing  [todo]"))
        .stdout(contains("03  Third thing  [todo]"));
}

#[test]
fn status_shows_state_agents_and_pending_events() {
    let r = repo();
    set_state(
        &r,
        serde_json::json!({
            "slug": "demo",
            "tickets": {"01": {"status": "doing", "assignee": "worker.1", "pair": 1}},
            "agents": {"worker.1": {"role": "worker", "index": 1, "busy": true, "turns": 2}},
            "events": [
                {"seq": 1, "kind": "blocked", "ticket": "02", "text": "old"},
                {"seq": 2, "kind": "passed", "ticket": "01", "text": "fresh"}],
            "events_consumed": 1,
            "next_seq": 2
        }),
    );
    r.wteam()
        .arg("status")
        .assert()
        .success()
        .stdout(contains("01  First thing  [doing]  worker.1"))
        .stdout(contains("worker.1  busy"))
        .stdout(contains("fresh"))
        .stdout(contains("old").not());

    let v = json_of(&r, &["status"]);
    assert_eq!(v["tickets"][0]["status"], "doing");
    assert_eq!(v["tickets"][0]["assignee"], "worker.1");
    assert_eq!(v["tickets"][1]["status"], "todo");
    assert_eq!(v["agents"][0]["name"], "worker.1");
    assert_eq!(v["events"].as_array().unwrap().len(), 1);
    assert_eq!(v["events"][0]["text"], "fresh");
}

#[test]
fn status_names_every_ticket_status() {
    let r = repo();
    set_state(
        &r,
        serde_json::json!({"slug": "demo", "tickets": {
            "01": {"status": "awaiting_approval"}, "02": {"status": "review"},
            "03": {"status": "blocked"}}}),
    );
    let v = json_of(&r, &["status"]);
    let names: Vec<&str> = v["tickets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["status"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["awaiting_approval", "review", "blocked"]);
}

//! `wteam requeue`: blocked tickets return to todo with an orchestrator note.

mod support;

use predicates::str::contains;
use serde_json::Value;
use support::Repo;

/// Worker that records each brief, then blocks on the first turn only.
const WORKER: &str = r#"
printf '%s\n=====\n' "$WTEAM_PROMPT" >> "$WTEAM_FAKE_DIR/worker-prompts"
if [ ! -e "$WTEAM_FAKE_DIR/blocked-once" ]; then
  touch "$WTEAM_FAKE_DIR/blocked-once"
  wteam block 01 --reason "need schema"
fi
"#;

fn repo() -> Repo {
    let r = Repo::new("demo");
    r.ticket("01", "Add thing", "None", "src/a.txt", "true");
    r.use_fake_agents();
    r.agent_script("worker", WORKER);
    r
}

fn block_01(r: &Repo) {
    r.wteam().args(["dispatch", "01"]).assert().success();
    r.wait_for(30, |s| {
        s["tickets"]["01"]["status"] == "blocked" && s["agents"]["worker.1"]["busy"] == false
    });
}

#[test]
fn requeue_resets_blocked_ticket_and_dispatch_brief_carries_note() {
    let r = repo();
    block_01(&r);
    r.wteam()
        .args(["requeue", "01", "--note", "schema lives in db/schema.sql"])
        .assert()
        .success();

    let t = r.state()["tickets"]["01"].clone();
    assert_eq!(t["status"], "todo");
    assert_eq!(t["rounds"], 0);
    assert!(t["assignee"].is_null() && t["pair"].is_null() && t["blocked_reason"].is_null());
    assert_eq!(t["note"], "schema lives in db/schema.sql");

    // Ready again.
    let out = r.wteam().args(["--json", "ready"]).output().unwrap();
    let ready: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(ready[0]["id"], "01");

    r.wteam().args(["dispatch", "01"]).assert().success();
    r.wait_for(30, |s| {
        // Turn 2 is the requeued brief. The silent worker is then re-prompted and blocked,
        // so `turns` may already be past 2 when polled; the brief is what matters here.
        s["agents"]["worker.1"]["turns"].as_u64().unwrap_or(0) >= 2
    });
    let prompts = std::fs::read_to_string(r.fake_dir().join("worker-prompts")).unwrap();
    let second = prompts.split("=====\n").nth(1).unwrap_or_default();
    assert!(
        second.contains("Orchestrator note") && second.contains("schema lives in db/schema.sql"),
        "{prompts}"
    );
}

#[test]
fn requeue_without_note_clears_the_old_note() {
    let r = repo();
    block_01(&r);
    r.wteam()
        .args(["requeue", "01", "--note", "first"])
        .assert()
        .success();
    r.wteam().args(["dispatch", "01"]).assert().success();
    r.wait_for(30, |s| s["agents"]["worker.1"]["busy"] == false);
    r.wteam()
        .args(["block", "01", "--reason", "again"])
        .assert()
        .success();
    r.wteam().args(["requeue", "01"]).assert().success();
    assert!(r.state()["tickets"]["01"]["note"].is_null());
}

#[test]
fn requeue_json_returns_new_state() {
    let r = repo();
    block_01(&r);
    let out = r
        .wteam()
        .args(["--json", "requeue", "01", "--note", "try again"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["ticket"], "01");
    assert_eq!(v["status"], "todo");
    assert_eq!(v["rounds"], 0);
    assert_eq!(v["note"], "try again");
}

#[test]
fn requeue_rejects_every_status_but_blocked() {
    let r = repo();
    for status in ["todo", "doing", "review", "awaiting_approval", "done"] {
        r.write(
            &format!(".wteam/{}/state.json", r.slug),
            &serde_json::json!({"slug": "demo", "tickets": {"01": {"status": status}}}).to_string(),
        );
        r.wteam()
            .args(["requeue", "01"])
            .assert()
            .failure()
            .stderr(contains(status));
    }
}

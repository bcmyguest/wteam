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

#[test]
fn requeue_note_file_carries_shell_metacharacters_into_the_brief() {
    let r = repo();
    block_01(&r);
    let note = "use $(echo X) && `rm -rf z` | cat ; \"quoted\"\nsecond line";
    r.write(".wteam/demo/requeue-01.md", note);
    let path = r.path().join(".wteam/demo/requeue-01.md");
    r.wteam()
        .args(["requeue", "01", "--note-file"])
        .arg(&path)
        .assert()
        .success();
    assert_eq!(r.state()["tickets"]["01"]["note"], note);

    r.wteam().args(["dispatch", "01"]).assert().success();
    r.wait_for(30, |s| {
        s["agents"]["worker.1"]["turns"].as_u64().unwrap_or(0) >= 2
    });
    let prompts = std::fs::read_to_string(r.fake_dir().join("worker-prompts")).unwrap();
    let second = prompts.split("=====\n").nth(1).unwrap_or_default();
    assert!(second.contains(note), "{prompts}");
}

#[test]
fn requeue_note_and_note_file_together_is_a_usage_error() {
    let r = repo();
    block_01(&r);
    r.write(".wteam/demo/requeue-01.md", "from file");
    let path = r.path().join(".wteam/demo/requeue-01.md");
    r.wteam()
        .args(["requeue", "01", "--note", "inline", "--note-file"])
        .arg(&path)
        .assert()
        .failure()
        .code(2)
        .stderr(contains("cannot be used with"));
    assert_eq!(r.state()["tickets"]["01"]["status"], "blocked");
}

/// Worker under worktree isolation: the first turn leaves `src/wip.txt` uncommitted and blocks
/// (the block keeps it as a wip commit). Later turns record HEAD and the files in the worktree.
const WIP_WORKER: &str = r#"
if [ ! -e "$WTEAM_FAKE_DIR/blocked-once" ]; then
  touch "$WTEAM_FAKE_DIR/blocked-once"
  mkdir -p src
  echo wip > src/wip.txt
  wteam block 01 --reason "need schema" || exit 1
  exit 0
fi
printf '%s' "$WTEAM_SESSION" > "$WTEAM_FAKE_DIR/session"
if [ -n "$WTEAM_ROLE_PROMPT" ]; then echo yes > "$WTEAM_FAKE_DIR/role"; fi
git rev-parse HEAD > "$WTEAM_FAKE_DIR/head"
ls src > "$WTEAM_FAKE_DIR/files" 2>/dev/null
"#;

/// Block `01` on pair 1 with a wip commit on its branch, under worktree isolation. The worker
/// and a reviewer each hold a session on `01`. Returns once the worker is idle again.
fn blocked_wip_repo() -> Repo {
    let r = Repo::new("demo");
    r.ticket("01", "Add thing", "None", "src/a.txt", "true");
    r.use_fake_agents();
    r.agent_script("orchestrator", "echo ok\n");
    r.agent_script("planner", "echo ok\n");
    r.write(
        ".wteam/demo/state.json",
        r#"{"slug":"demo","run":{"pairs":1,"isolation":"worktree","transport":"resume","team":"default"}}"#,
    );
    r.agent_script("worker", WIP_WORKER);
    r.wteam().args(["dispatch", "01"]).assert().success();
    r.wait_for(60, |s| {
        s["tickets"]["01"]["status"] == "blocked" && s["agents"]["worker.1"]["busy"] == false
    });
    r
}

/// Block `01` with a wip commit on its branch, requeue it (with `--fresh` when asked) and
/// dispatch it again. Returns once the second turn has recorded the worktree.
fn wip_then_requeue(fresh: bool) -> Repo {
    let r = blocked_wip_repo();
    let args: &[&str] = if fresh {
        &["requeue", "01", "--fresh"]
    } else {
        &["requeue", "01"]
    };
    r.wteam().args(args).assert().success();
    r.wteam().args(["dispatch", "01"]).assert().success();
    r.wait_for(60, |_| r.fake_dir().join("head").exists());
    r
}

/// Rewrite `state.json` through `edit`, to stage a state no run would reach by itself.
fn edit_state(r: &Repo, edit: impl FnOnce(&mut Value)) {
    let mut s = r.state();
    edit(&mut s);
    r.write(".wteam/demo/state.json", &s.to_string());
}

/// Branch sha and worktree presence for `01`: what `--fresh` must not touch when it refuses.
fn preserved_work(r: &Repo) -> (String, bool) {
    let sha = r.git(&["rev-parse", "wteam/demo/01"]);
    (sha, r.path().join(".wteam/demo/worktrees/pair1").is_dir())
}

#[test]
fn requeue_fresh_dispatches_from_main_without_the_wip() {
    let r = wip_then_requeue(true);
    assert_eq!(
        r.read(".wteam-fake/head").trim(),
        r.git(&["rev-parse", "HEAD"]).trim(),
        "next dispatch did not start from main"
    );
    assert!(!r.read(".wteam-fake/files").contains("wip.txt"));
}

#[test]
fn requeue_without_fresh_keeps_the_wip_for_the_next_dispatch() {
    let r = wip_then_requeue(false);
    assert_ne!(
        r.read(".wteam-fake/head").trim(),
        r.git(&["rev-parse", "HEAD"]).trim(),
        "wip commit was discarded"
    );
    assert!(r.read(".wteam-fake/files").contains("wip.txt"));
    assert!(
        !r.read(".wteam-fake/session").is_empty(),
        "session was dropped"
    );
    assert!(r.read(".wteam-fake/role").is_empty(), "role prompt resent");
}

#[test]
fn requeue_fresh_clears_the_sessions_bound_to_the_ticket() {
    let r = blocked_wip_repo();
    assert_eq!(r.state()["agents"]["worker.1"]["session_ticket"], "01");
    edit_state(&r, |s| {
        s["agents"]["reviewer.1"] = serde_json::json!({
            "role": "reviewer", "index": 1, "session_id": "R1",
            "session_turns": 2, "session_ticket": "01"
        });
    });
    r.wteam()
        .args(["requeue", "01", "--fresh"])
        .assert()
        .success();
    let st = r.state();
    for agent in ["worker.1", "reviewer.1"] {
        let a = &st["agents"][agent];
        assert!(a["session_id"].is_null(), "{agent} kept its session");
        assert_eq!(a["session_turns"], 0, "{agent}");
        assert!(a["session_ticket"].is_null(), "{agent}");
    }
}

#[test]
fn requeue_fresh_next_turn_starts_a_new_session_with_the_role_prompt() {
    let r = wip_then_requeue(true);
    assert!(r.read(".wteam-fake/session").is_empty(), "session resumed");
    assert_eq!(r.read(".wteam-fake/role").trim(), "yes");
}

#[test]
fn requeue_fresh_refuses_when_another_ticket_holds_the_pair() {
    let r = blocked_wip_repo();
    edit_state(&r, |s| {
        s["tickets"]["02"] = serde_json::json!({"status": "doing", "pair": 1});
    });
    let before = r.state();
    let work = preserved_work(&r);
    assert!(work.1, "worktree missing before the refusal");
    r.wteam()
        .args(["requeue", "01", "--fresh"])
        .assert()
        .failure()
        .stderr(contains("pair 1 is busy with 02"));
    assert_eq!(r.state(), before);
    assert_eq!(preserved_work(&r), work);
}

#[test]
fn requeue_fresh_refuses_while_the_assignee_runs_a_turn() {
    let r = blocked_wip_repo();
    edit_state(&r, |s| {
        s["agents"]["worker.1"]["busy"] = serde_json::json!(true)
    });
    let before = r.state();
    let work = preserved_work(&r);
    assert!(work.1, "worktree missing before the refusal");
    r.wteam()
        .args(["requeue", "01", "--fresh"])
        .assert()
        .failure()
        .stderr(contains("worker.1 is still running a turn for 01"));
    assert_eq!(r.state(), before);
    assert_eq!(preserved_work(&r), work);
}

#[test]
fn requeue_fresh_on_a_ticket_that_is_not_blocked_changes_nothing() {
    let r = repo();
    let before = r.state();
    r.wteam()
        .args(["requeue", "01", "--fresh"])
        .assert()
        .failure()
        .stderr(contains("only blocked tickets"));
    assert_eq!(r.state(), before);
}

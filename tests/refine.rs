//! Black-box tests for `wteam refine`.

mod support;

use support::Repo;

#[test]
fn refine_reports_stale_cursor_tree_and_blocked_ticket() {
    let r = Repo::new("bench");
    r.write(
        "src/adapters/mod.rs",
        "pub fn x() { \"the cursor adapter is not implemented yet (second wave); its permissions translate\" }\n",
    );
    r.write(".wteam/bench/state.json", r#"{
  "slug": "bench",
  "tickets": {
    "02": {
      "status": "blocked",
      "blocked_reason": "reviewer.2 turn failed: the cursor adapter is not implemented yet (second wave); its permissions translate"
    }
  },
  "agents": {},
  "run": { "pairs": 1, "isolation": "none", "transport": "resume", "team": "default", "dispatch": "auto" }
}"#);
    r.write(".wteam/bench/tickets/02-watcher.md", "# 02: W\n\n**What to build:** x\n\n**Blocked by:** None\n\n**Status:** ready-for-agent\n\n- [ ] ok\n");
    let logs = r.path().join("bench-logs");
    std::fs::create_dir_all(&logs).unwrap();
    std::fs::write(
        logs.join("run.txt"),
        "Run stopped: ready tickets were not dispatched.\n",
    )
    .unwrap();

    r.wteam()
        .args([
            "--slug",
            "bench",
            "refine",
            "--logs",
            logs.to_str().unwrap(),
            "--run-log",
            logs.join("run.txt").to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("stale cursor error"))
        .stdout(predicates::str::contains(
            "Checkout source still advertises cursor",
        ));

    r.wteam()
        .args([
            "--slug",
            "bench",
            "refine",
            "--logs",
            logs.to_str().unwrap(),
            "--apply",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("requeued ticket 02"));

    let state: serde_json::Value =
        serde_json::from_str(&r.read(".wteam/bench/state.json")).unwrap();
    assert_eq!(state["tickets"]["02"]["status"], "todo");
}

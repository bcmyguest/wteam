//! The dispatch -> submit -> verdict -> commit loop through the binary, with fake agents.

mod support;

use serde_json::Value;
use support::Repo;

/// Worker that appends to `src/a.txt`, leaves an unrelated file, and submits ticket 01.
const WORKER: &str = r#"
echo "$WTEAM_SESSION" >> "$WTEAM_FAKE_DIR/worker-sessions"
printf '%s\n=====\n' "$WTEAM_PROMPT" >> "$WTEAM_FAKE_DIR/worker-prompts"
mkdir -p src
echo edit >> src/a.txt
echo scratch > scratch.txt
wteam submit 01 --summary "edited a.txt" || exit 1
echo "session: S1"
echo "tokens: 1234"
"#;

fn repo() -> Repo {
    let r = Repo::new("demo");
    r.ticket("01", "Add thing", "None", "src/a.txt", "cargo test");
    r.use_fake_agents();
    r
}

fn events(r: &Repo) -> Vec<Value> {
    let out = r
        .wteam()
        .args(["--json", "wait", "--timeout", "30"])
        .assert()
        .success();
    let v: Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    v["events"].as_array().unwrap().clone()
}

fn dispatch(r: &Repo, ticket: &str) {
    r.wteam().args(["dispatch", ticket]).assert().success();
}

fn files_of(r: &Repo, rev: &str) -> Vec<String> {
    r.git(&["show", "--name-only", "--format=", rev])
        .lines()
        .map(str::to_string)
        .collect()
}

#[test]
fn happy_path_commits_only_ticket_files() {
    let r = repo();
    r.agent_script("worker", WORKER);
    r.agent_script(
        "reviewer",
        "printf '%s' \"$WTEAM_PROMPT\" > \"$WTEAM_FAKE_DIR/review-request\"\nwteam verdict 01 pass\n",
    );
    dispatch(&r, "01");
    let ev = events(&r);
    assert_eq!(ev.len(), 1, "{ev:?}");
    assert_eq!(ev[0]["kind"], "passed");
    assert_eq!(ev[0]["ticket"], "01");

    let head = r.git(&["rev-parse", "HEAD"]).trim().to_string();
    assert_eq!(
        r.git(&["log", "-1", "--format=%s"]).trim(),
        "feat: Add thing (wteam 01)"
    );
    assert_eq!(files_of(&r, "HEAD"), vec!["src/a.txt"]);
    let st = r.state();
    assert_eq!(st["tickets"]["01"]["status"], "done");
    assert_eq!(st["tickets"]["01"]["commit"], head.as_str());

    // The reviewer saw the worker's summary and a diff that includes the new file.
    let req = std::fs::read_to_string(r.fake_dir().join("review-request")).unwrap();
    assert!(
        req.contains("edited a.txt") && req.contains("+edit"),
        "{req}"
    );
    assert!(!req.contains("scratch"), "{req}");

    // Turn bookkeeping: session reported, tokens, turn count, one log per turn.
    let w = &st["agents"]["worker.1"];
    assert_eq!(w["session_id"], "S1");
    assert_eq!(w["context_tokens"], 1234);
    assert_eq!(w["turns"], 1);
    assert_eq!(w["busy"], false);
    assert!(r.effort().join("logs/worker.1-1.log").is_file());
    assert!(r.effort().join("logs/reviewer.1-1.log").is_file());
}

#[test]
fn missing_session_line_invents_fake_session() {
    let r = repo();
    r.agent_script("worker", "wteam submit 01\n");
    r.agent_script("reviewer", "wteam verdict 01 pass\n");
    dispatch(&r, "01");
    events(&r);
    assert_eq!(
        r.state()["agents"]["worker.1"]["session_id"],
        "fake-worker.1"
    );
}

#[test]
fn fail_returns_findings_then_resumes_session_and_passes() {
    let r = repo();
    r.agent_script("worker", WORKER);
    r.agent_script(
        "reviewer",
        r#"
printf '%s\n=====\n' "$WTEAM_PROMPT" >> "$WTEAM_FAKE_DIR/reviewer-prompts"
if [ -e "$WTEAM_FAKE_DIR/failed-once" ]; then
  wteam verdict 01 pass
else
  touch "$WTEAM_FAKE_DIR/failed-once"
  wteam verdict 01 fail --finding "handle the empty case"
fi
"#,
    );
    dispatch(&r, "01");
    let ev = events(&r);
    assert_eq!(ev[0]["kind"], "passed", "{ev:?}");

    let dir = r.fake_dir();
    let sessions = std::fs::read_to_string(dir.join("worker-sessions")).unwrap();
    assert_eq!(
        sessions.lines().collect::<Vec<_>>(),
        vec!["", "S1"],
        "resume uses the first turn's id"
    );
    let prompts = std::fs::read_to_string(dir.join("worker-prompts")).unwrap();
    let rounds: Vec<&str> = prompts.split("=====").collect();
    assert!(rounds[1].contains("handle the empty case"), "{prompts}");
    let reviews = std::fs::read_to_string(dir.join("reviewer-prompts")).unwrap();
    assert_eq!(
        reviews.matches("Review request: ticket 01").count(),
        2,
        "{reviews}"
    );

    let st = r.state();
    assert_eq!(st["tickets"]["01"]["rounds"], 1);
    assert_eq!(st["agents"]["worker.1"]["turns"], 2);
    assert!(r.effort().join("logs/worker.1-2.log").is_file());
}

#[test]
fn adapter_failure_blocks_the_ticket() {
    let r = repo();
    r.agent_script("worker", "echo boom >&2\nexit 3\n");
    dispatch(&r, "01");
    let ev = events(&r);
    assert_eq!(ev[0]["kind"], "blocked", "{ev:?}");
    assert_eq!(ev[0]["ticket"], "01");
    let st = r.state();
    let reason = st["tickets"]["01"]["blocked_reason"].as_str().unwrap();
    assert!(reason.contains("boom"), "{reason}");
    assert_eq!(st["tickets"]["01"]["status"], "blocked");
    assert_eq!(st["agents"]["worker.1"]["busy"], false);
    let log = std::fs::read_to_string(r.effort().join("logs/worker.1-1.log")).unwrap();
    assert!(log.contains("boom"));
    let mail = std::fs::read_to_string(r.effort().join("mail/orchestrator.1.md")).unwrap();
    assert!(mail.contains("boom"), "{mail}");
}

#[test]
fn turn_is_detached_and_a_busy_agent_reuses_its_loop() {
    let r = repo();
    r.agent_script(
        "worker",
        r#"
echo $PPID >> "$WTEAM_FAKE_DIR/ppids"
touch "$WTEAM_FAKE_DIR/started"
sleep 1
echo "$WTEAM_PROMPT" >> "$WTEAM_FAKE_DIR/prompts"
touch "$WTEAM_FAKE_DIR/finished"
"#,
    );
    r.wteam()
        .args(["send", "worker.1", "first"])
        .assert()
        .success();
    // `send` returned while the turn is still sleeping.
    assert!(!r.fake_dir().join("finished").exists());
    r.wait_for(10, |_| r.fake_dir().join("started").exists());
    r.wteam()
        .args(["send", "worker.1", "second"])
        .assert()
        .success();
    r.wait_for(20, |s| {
        s["agents"]["worker.1"]["turns"] == 2 && s["agents"]["worker.1"]["busy"] == false
    });
    let ppids = std::fs::read_to_string(r.fake_dir().join("ppids")).unwrap();
    let ppids: Vec<&str> = ppids.lines().collect();
    assert_eq!(ppids.len(), 2);
    assert_eq!(ppids[0], ppids[1], "both turns ran in one __turn process");
    let prompts = std::fs::read_to_string(r.fake_dir().join("prompts")).unwrap();
    assert!(
        prompts.contains("first") && prompts.contains("second"),
        "{prompts}"
    );
}

#[test]
fn verdict_rules_are_enforced() {
    let r = repo();
    r.agent_script("worker", "wteam submit 01\n");
    // Holds its turn until the test has checked the rules, so the silent-turn
    // re-prompt cannot race the verdicts below.
    r.agent_script(
        "reviewer",
        "i=0\nwhile [ ! -f \"$WTEAM_FAKE_DIR/go\" ] && [ $i -lt 200 ]; do sleep 0.1; i=$((i+1)); done\n",
    );
    dispatch(&r, "01");
    r.wait_for(20, |s| {
        s["tickets"]["01"]["status"] == "review" && s["agents"]["worker.1"]["busy"] == false
    });
    let verdict = |agent: &str, args: &[&str]| {
        r.wteam()
            .env("WTEAM_AGENT", agent)
            .args(["verdict", "01"])
            .args(args)
            .assert()
    };
    verdict("worker.1", &["pass"]).failure();
    verdict("human", &["pass"]).failure();
    verdict("reviewer.1", &["pass", "--finding", "nit"]).failure();
    verdict("reviewer.1", &["fail"]).failure();
    assert_eq!(r.state()["tickets"]["01"]["status"], "review");
    verdict("reviewer.1", &["pass"]).success();
    assert_eq!(r.state()["tickets"]["01"]["status"], "done");
    r.write(".wteam-fake/go", "");
}

#[test]
fn submit_requires_the_assignee_and_dispatch_requires_ready() {
    let r = repo();
    r.ticket("02", "Next thing", "01", "src/b.txt", "");
    r.agent_script("worker", "true\n");
    r.wteam().args(["dispatch", "02"]).assert().failure();
    dispatch(&r, "01");
    r.wteam().args(["submit", "01"]).assert().failure();
    r.wteam()
        .env("WTEAM_AGENT", "worker.2")
        .args(["submit", "01"])
        .assert()
        .failure();
}

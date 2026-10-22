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
    r.ticket("01", "Add thing", "None", "src/a.txt", "true");
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

/// Worker that edits its Files plus `docs/extra.txt` (declared with `--extra`) and leaves `scratch.txt`.
const EXTRA_WORKER: &str = r#"
mkdir -p src docs
echo edit >> src/a.txt
echo extra > docs/extra.txt
echo scratch > scratch.txt
wteam submit 01 --summary "docs/extra.txt is needed for the example" --extra docs/extra.txt || exit 1
echo "session: S1"
echo "tokens: 1234"
"#;

#[test]
fn extra_paths_are_listed_for_review_and_committed_on_pass() {
    let r = repo();
    r.agent_script("worker", EXTRA_WORKER);
    r.agent_script(
        "reviewer",
        "printf '%s' \"$WTEAM_PROMPT\" > \"$WTEAM_FAKE_DIR/review-request\"\nwteam verdict 01 pass\n",
    );
    dispatch(&r, "01");
    let ev = events(&r);
    assert_eq!(ev.len(), 1, "{ev:?}");
    assert_eq!(ev[0]["kind"], "passed");

    let req = r.read(".wteam-fake/review-request");
    assert!(req.contains("Extra paths outside Files"), "{req}");
    assert!(req.contains("- docs/extra.txt"), "{req}");

    let files = files_of(&r, "HEAD");
    assert!(files.contains(&"src/a.txt".to_string()), "{files:?}");
    assert!(files.contains(&"docs/extra.txt".to_string()), "{files:?}");
    assert!(!files.contains(&"scratch.txt".to_string()), "{files:?}");
}

#[test]
fn extra_path_inside_another_in_progress_tickets_files_is_refused() {
    let r = repo();
    r.ticket("02", "Add two", "None", "src/b.txt", "");
    r.write(
        ".wteam/demo/state.json",
        r#"{"slug":"demo","run":{"pairs":2,"isolation":"none","transport":"resume","team":"default"}}"#,
    );
    // Both workers hold their tickets (no submit) until `go` appears, so both stay in progress.
    r.agent_script(
        "worker",
        "i=0; while [ ! -f \"$WTEAM_FAKE_DIR/go\" ] && [ $i -lt 300 ]; do sleep 0.1; i=$((i+1)); done\n",
    );
    dispatch(&r, "01");
    r.wteam().args(["dispatch", "02"]).assert().success();

    r.wteam()
        .env("WTEAM_AGENT", "worker.2")
        .args(["submit", "02", "--extra", "src/a.txt"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("files of ticket 01"));
}

/// `submit` with one `--extra` value must fail before any state change, naming the value and the rule.
fn extra_is_rejected(extra: &str, rule: &str) {
    let r = repo();
    r.wteam()
        .args(["submit", "01", "--extra", extra])
        .assert()
        .failure()
        .stderr(predicates::str::contains(format!(
            "--extra `{extra}` {rule}"
        )));
}

#[test]
fn extra_rejects_absolute_path() {
    extra_is_rejected("/etc/passwd", "must be relative to the repo root");
}

#[test]
fn extra_rejects_parent_component() {
    extra_is_rejected("../x", "must not contain `..`");
}

#[test]
fn extra_rejects_glob() {
    extra_is_rejected("*", "must be a literal path, not a glob");
}

#[test]
fn extra_rejects_repo_root() {
    extra_is_rejected(".", "must name a file, not the repo root");
}

#[test]
fn extra_rejects_empty() {
    extra_is_rejected("", "must name a file, not the repo root");
}

#[test]
fn extra_rejects_wteam_dir() {
    extra_is_rejected(".wteam/demo/state.json", "must not be under .wteam/");
}

#[test]
fn deleted_tracked_file_declared_as_extra_is_committed_as_deletion() {
    let r = repo();
    r.write("docs/old.txt", "old\n");
    r.git(&["add", "-A"]);
    r.git(&["commit", "-q", "-m", "chore: old doc"]);
    r.agent_script(
        "worker",
        r#"
mkdir -p src
echo edit >> src/a.txt
rm docs/old.txt
wteam submit 01 --summary "removed docs/old.txt" --extra docs/old.txt || exit 1
echo "session: S1"
"#,
    );
    r.agent_script("reviewer", "wteam verdict 01 pass\n");
    dispatch(&r, "01");
    let ev = events(&r);
    assert_eq!(ev[0]["kind"], "passed", "{ev:?}");

    let status = r.git(&["show", "--format=", "--name-status", "HEAD"]);
    assert!(status.contains("D\tdocs/old.txt"), "{status}");
    assert!(!r.path().join("docs/old.txt").exists());
}

#[test]
fn dispatch_clears_extras_from_the_previous_round() {
    let r = repo();
    r.write(
        ".wteam/demo/state.json",
        r#"{"slug":"demo","tickets":{"01":{"status":"blocked","extras":["docs/x.txt"],"blocked_reason":"old"}},"run":{"pairs":1,"isolation":"none","transport":"resume","team":"default"}}"#,
    );
    r.agent_script(
        "worker",
        "i=0; while [ ! -f \"$WTEAM_FAKE_DIR/go\" ] && [ $i -lt 300 ]; do sleep 0.1; i=$((i+1)); done\n",
    );
    r.wteam().args(["requeue", "01"]).assert().success();
    dispatch(&r, "01");
    let s = r.state();
    assert_eq!(s["tickets"]["01"]["extras"], serde_json::json!([]), "{s:#}");
    r.write(".wteam-fake/go", "");
}

#[test]
fn extra_rejects_top_magic_pathspec() {
    extra_is_rejected(
        ":(top).wteam/x",
        "must not start with `:` (git pathspec magic)",
    );
}

#[test]
fn extra_rejects_root_magic_pathspec() {
    extra_is_rejected(":/.wteam/x", "must not start with `:` (git pathspec magic)");
}

#[test]
fn extra_rejects_path_through_symlink() {
    let r = repo();
    std::os::unix::fs::symlink(".wteam", r.path().join("link")).unwrap();
    r.wteam()
        .args(["submit", "01", "--extra", "link/state.json"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "--extra `link/state.json` passes through symlink `link`",
        ));
}

#[test]
fn multi_line_summary_reaches_review_request() {
    let r = repo();
    r.agent_script(
        "worker",
        r#"
mkdir -p src
echo edit >> src/a.txt
wteam submit 01 --summary "$(printf 'edited a.txt\nsecond line\nthird line')" || exit 1
echo "session: S1"
"#,
    );
    r.agent_script(
        "reviewer",
        "printf '%s' \"$WTEAM_PROMPT\" > \"$WTEAM_FAKE_DIR/review-request\"\nwteam verdict 01 pass\n",
    );
    dispatch(&r, "01");
    assert_eq!(events(&r)[0]["kind"], "passed");
    let req = std::fs::read_to_string(r.fake_dir().join("review-request")).unwrap();
    assert!(
        req.contains("edited a.txt\nsecond line\nthird line"),
        "{req}"
    );
}

/// Holds the agent's turn until the test writes `$WTEAM_FAKE_DIR/go`.
const HOLD: &str = "i=0\nwhile [ ! -f \"$WTEAM_FAKE_DIR/go\" ] && [ $i -lt 200 ]; do sleep 0.1; i=$((i+1)); done\n";

#[test]
fn submit_summary_file_round_trips_byte_exact() {
    let r = repo();
    // Outer whitespace is trimmed by the engine; the inner content must survive verbatim.
    let expected = "str<->int; a | b $(x) `tick`\n\nmiddle line  \n  indented; a|b;c\nsecond line";
    let summary = format!("\n\n{expected}\n\n");
    r.write("scratch/summary-01.md", &summary);
    r.agent_script(
        "worker",
        "mkdir -p src\necho edit >> src/a.txt\nwteam submit 01 --summary-file scratch/summary-01.md || exit 1\necho \"session: S1\"\necho \"tokens: 1\"\n",
    );
    r.agent_script(
        "reviewer",
        "printf '%s' \"$WTEAM_PROMPT\" > \"$WTEAM_FAKE_DIR/review-request\"\nwteam verdict 01 pass\n",
    );
    dispatch(&r, "01");
    events(&r);
    let req = r.read(".wteam-fake/review-request");
    let start = req.find("Worker summary:\n").expect("summary header") + "Worker summary:\n".len();
    let end = start + expected.len();
    assert_eq!(req.get(start..end), Some(expected), "{req}");
    assert_eq!(req.as_bytes().get(end), Some(&b'\n'), "{req}");
}

#[test]
fn submit_rejects_both_summary_flags() {
    let r = repo();
    r.wteam()
        .args(["submit", "01", "--summary", "x", "--summary-file", "y.md"])
        .assert()
        .failure();
}

#[test]
fn submit_missing_summary_file_leaves_state_unchanged() {
    let r = repo();
    r.agent_script("worker", HOLD);
    dispatch(&r, "01");
    r.wait_for(20, |s| {
        s["tickets"]["01"]["status"] == "doing" && s["agents"]["worker.1"]["busy"] == true
    });
    r.wteam()
        .env("WTEAM_AGENT", "worker.1")
        .args(["submit", "01", "--summary-file", "nope.md"])
        .assert()
        .failure();
    let st = r.state();
    assert_eq!(st["tickets"]["01"]["status"], "doing");
    assert_eq!(st["tickets"]["01"]["assignee"], "worker.1");
    r.write(".wteam-fake/go", "");
}

#[test]
fn block_reason_file_round_trips() {
    let r = repo();
    r.write("scratch/block-01.md", "needs <-> schema; $(x)\n");
    r.agent_script(
        "worker",
        "wteam block 01 --reason-file scratch/block-01.md || exit 1\n",
    );
    dispatch(&r, "01");
    let st = r.wait_for(20, |s| s["tickets"]["01"]["status"] == "blocked");
    assert_eq!(
        st["tickets"]["01"]["blocked_reason"],
        "needs <-> schema; $(x)"
    );
}

#[test]
fn block_needs_exactly_one_reason_source() {
    let r = repo();
    r.wteam().args(["block", "01"]).assert().failure();
    r.wteam()
        .args(["block", "01", "--reason", "x", "--reason-file", "y.md"])
        .assert()
        .failure();
}

#[test]
fn block_missing_reason_file_leaves_state_unchanged() {
    let r = repo();
    r.agent_script("worker", HOLD);
    dispatch(&r, "01");
    r.wait_for(20, |s| {
        s["tickets"]["01"]["status"] == "doing" && s["agents"]["worker.1"]["busy"] == true
    });
    r.wteam()
        .env("WTEAM_AGENT", "worker.1")
        .args(["block", "01", "--reason-file", "nope.md"])
        .assert()
        .failure();
    assert_eq!(r.state()["tickets"]["01"]["status"], "doing");
    r.write(".wteam-fake/go", "");
}

#[test]
fn verdict_file_pass_takes_notes_and_follow_ups() {
    let r = repo();
    r.agent_script("worker", "wteam submit 01 --summary ok || exit 1\n");
    r.agent_script("reviewer", HOLD);
    dispatch(&r, "01");
    r.wait_for(20, |s| {
        s["tickets"]["01"]["status"] == "review" && s["agents"]["reviewer.1"]["busy"] == true
    });
    r.write(
        "scratch/verdict-01.md",
        "FINDINGS:\n\nNOTES:\n- rename the helper <-> later\n\nFOLLOW-UPS:\n- dup: shared helper; see $(x)\n",
    );
    r.wteam()
        .env("WTEAM_AGENT", "reviewer.1")
        .args(["verdict", "01", "pass", "--file", "scratch/verdict-01.md"])
        .assert()
        .success();
    assert_eq!(r.state()["tickets"]["01"]["status"], "done");
    let reviews = r.read(".wteam/demo/reviews/01.md");
    assert!(reviews.contains("rename the helper <-> later"), "{reviews}");
    assert!(
        r.read(".wteam/demo/queue.md")
            .contains("shared helper; see $(x)")
    );
    r.write(".wteam-fake/go", "");
}

#[test]
fn verdict_file_fail_merges_with_flags() {
    let r = repo();
    r.agent_script("worker", "wteam submit 01 --summary ok || exit 1\n");
    r.agent_script("reviewer", HOLD);
    dispatch(&r, "01");
    r.wait_for(20, |s| {
        s["tickets"]["01"]["status"] == "review" && s["agents"]["reviewer.1"]["busy"] == true
    });
    r.write(
        "scratch/verdict-01.md",
        "FINDINGS:\n- src/a.txt: handle <-> empty\n",
    );
    r.wteam()
        .env("WTEAM_AGENT", "reviewer.1")
        .args([
            "verdict",
            "01",
            "fail",
            "--file",
            "scratch/verdict-01.md",
            "--finding",
            "from flag",
        ])
        .assert()
        .success();
    let reviews = r.read(".wteam/demo/reviews/01.md");
    assert!(reviews.contains("handle <-> empty"), "{reviews}");
    assert!(reviews.contains("from flag"), "{reviews}");
    assert_eq!(r.state()["tickets"]["01"]["rounds"], 1);
    r.write(".wteam-fake/go", "");
}

#[test]
fn verdict_file_missing_or_findingless_fail_leaves_state_unchanged() {
    let r = repo();
    r.agent_script("worker", "wteam submit 01 --summary ok || exit 1\n");
    r.agent_script("reviewer", HOLD);
    dispatch(&r, "01");
    r.wait_for(20, |s| {
        s["tickets"]["01"]["status"] == "review" && s["agents"]["reviewer.1"]["busy"] == true
    });
    r.write("scratch/verdict-01.md", "NOTES:\n- just a note\n");
    let verdict = |args: &[&str]| {
        r.wteam()
            .env("WTEAM_AGENT", "reviewer.1")
            .args(["verdict", "01"])
            .args(args)
            .assert()
    };
    verdict(&["fail", "--file", "nope.md"]).failure();
    verdict(&["fail", "--file", "scratch/verdict-01.md"]).failure();
    let st = r.state();
    assert_eq!(st["tickets"]["01"]["status"], "review");
    assert_eq!(st["tickets"]["01"]["rounds"], 0);
    r.write(".wteam-fake/go", "");
}

/// Worker for the session tests. Reads the ticket it works on from `task`. Each turn records
/// `session|role` (role marks a turn that carried the role prompt). With `block-01` present and
/// `blocked-once` absent, it blocks ticket 01 once instead of submitting.
const SESSION_WORKER: &str = r#"
t=$(cat "$WTEAM_FAKE_DIR/task")
printf '%s|%s\n' "$WTEAM_SESSION" "${WTEAM_ROLE_PROMPT:+role}" >> "$WTEAM_FAKE_DIR/worker-sessions"
if [ "$t" = 01 ] && [ -e "$WTEAM_FAKE_DIR/block-01" ] && [ ! -e "$WTEAM_FAKE_DIR/blocked-once" ]; then
  touch "$WTEAM_FAKE_DIR/blocked-once"
  wteam block 01 --reason "need schema"
  echo "session: S1"
  exit 0
fi
mkdir -p src
echo edit >> "src/$t.txt"
wteam submit "$t" --summary "did $t" || exit 1
echo "session: S1"
"#;

/// Reviewer that passes whatever ticket its request names and records `session|role` per turn.
/// It FAILs ticket 02 once when `fail-02` is set.
const SESSION_REVIEWER: &str = r#"
t=$(printf '%s' "$WTEAM_PROMPT" | sed -n 's/.*Review request: ticket \([0-9]*\).*/\1/p' | head -n 1)
printf '%s|%s\n' "$WTEAM_SESSION" "${WTEAM_ROLE_PROMPT:+role}" >> "$WTEAM_FAKE_DIR/reviewer-sessions"
if [ "$t" = 02 ] && [ -e "$WTEAM_FAKE_DIR/fail-02" ] && [ ! -e "$WTEAM_FAKE_DIR/failed-02" ]; then
  touch "$WTEAM_FAKE_DIR/failed-02"
  wteam verdict 02 fail --finding "handle the empty case" || exit 1
else
  wteam verdict "$t" pass || exit 1
fi
echo "session: R1"
"#;

fn session_repo() -> Repo {
    let r = Repo::new("demo");
    r.ticket("01", "First", "None", "src/01.txt", "true");
    r.ticket("02", "Second", "None", "src/02.txt", "true");
    r.use_fake_agents();
    r.agent_script("worker", SESSION_WORKER);
    r.agent_script("reviewer", SESSION_REVIEWER);
    r.write(".wteam-fake/task", "01");
    r
}

fn session_lines(r: &Repo, file: &str) -> Vec<String> {
    std::fs::read_to_string(r.fake_dir().join(file))
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

fn idle_and_done(r: &Repo, ticket: &str) {
    r.wait_for(30, |s| {
        s["tickets"][ticket]["status"] == "done"
            && s["agents"]["worker.1"]["busy"] == false
            && s["agents"]["reviewer.1"]["busy"] == false
    });
}

#[test]
fn new_ticket_starts_fresh_and_fail_fix_resumes_the_same_session() {
    let r = session_repo();
    r.write(".wteam-fake/fail-02", "");
    dispatch(&r, "01");
    idle_and_done(&r, "01");
    r.write(".wteam-fake/task", "02");
    dispatch(&r, "02");
    idle_and_done(&r, "02");

    // 01 starts fresh with the role prompt. 02 is a new ticket, so it starts fresh too. Its
    // fix turn after the FAIL resumes the session that now belongs to 02.
    assert_eq!(
        session_lines(&r, "worker-sessions"),
        vec!["|role", "|role", "S1|"]
    );
    // The reviewer's round 1 on 02 starts fresh; its round 2 resumes.
    assert_eq!(
        session_lines(&r, "reviewer-sessions"),
        vec!["|role", "|role", "R1|"]
    );
}

#[test]
fn requeued_ticket_resumes_its_session_when_no_other_ticket_ran() {
    let r = session_repo();
    r.write(".wteam-fake/block-01", "");
    dispatch(&r, "01");
    r.wait_for(30, |s| {
        s["tickets"]["01"]["status"] == "blocked" && s["agents"]["worker.1"]["busy"] == false
    });
    r.wteam()
        .args(["requeue", "01", "--note", "retry"])
        .assert()
        .success();
    dispatch(&r, "01");
    idle_and_done(&r, "01");

    // The blocked turn belonged to 01, so the requeued 01 resumes it.
    assert_eq!(session_lines(&r, "worker-sessions"), vec!["|role", "S1|"]);
}

#[test]
fn requeued_ticket_starts_fresh_when_its_worker_ran_another_ticket() {
    let r = session_repo();
    r.write(".wteam-fake/block-01", "");
    dispatch(&r, "01");
    r.wait_for(30, |s| {
        s["tickets"]["01"]["status"] == "blocked" && s["agents"]["worker.1"]["busy"] == false
    });
    r.write(".wteam-fake/task", "02");
    dispatch(&r, "02");
    idle_and_done(&r, "02");
    r.write(".wteam-fake/task", "01");
    r.wteam()
        .args(["requeue", "01", "--note", "retry"])
        .assert()
        .success();
    dispatch(&r, "01");
    idle_and_done(&r, "01");

    // The session now belongs to 02, so the requeued 01 starts fresh.
    assert_eq!(
        session_lines(&r, "worker-sessions"),
        vec!["|role", "|role", "|role"]
    );
}

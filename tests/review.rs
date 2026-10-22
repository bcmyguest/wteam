//! Review rounds, notes, follow-ups and the stop report, through the binary with fake agents.

mod support;

use serde_json::Value;
use support::Repo;

/// Worker that edits its ticket's file and submits; the ticket id comes from the brief or the failure mail.
const WORKER: &str = r#"
id=$(printf '%s\n' "$WTEAM_PROMPT" | sed -n 's/.*[Tt]icket \([0-9][0-9]\).*/\1/p' | head -1)
echo "$WTEAM_PROMPT" >> "$WTEAM_FAKE_DIR/worker-prompts"
mkdir -p src
echo edit >> "src/$id.txt"
wteam submit "$id" --summary "edited $id"
"#;

fn repo() -> Repo {
    let r = Repo::new("demo");
    r.use_fake_agents();
    r.agent_script("worker", WORKER);
    r
}

/// Consume pending events, waiting for at least one.
fn events(r: &Repo) -> Vec<Value> {
    let out = r
        .wteam()
        .args(["--json", "wait", "--timeout", "30"])
        .assert()
        .success();
    let v: Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    v["events"].as_array().unwrap().clone()
}

fn dispatch_and_wait(r: &Repo, id: &str) -> Vec<Value> {
    r.wteam().args(["dispatch", id]).assert().success();
    events(r)
}

fn report_text(r: &Repo) -> String {
    let out = r.wteam().arg("report").assert().success();
    String::from_utf8_lossy(&out.get_output().stdout).into_owned()
}

#[test]
fn third_fail_blocks_with_last_finding_and_leaves_independent_tickets_ready() {
    let r = repo();
    r.ticket("01", "Add thing", "None", "src/01.txt", "");
    r.ticket("02", "Other thing", "None", "src/02.txt", "");
    r.agent_script(
        "reviewer",
        r#"
n=$(cat "$WTEAM_FAKE_DIR/n" 2>/dev/null || echo 0)
n=$((n + 1))
echo $n > "$WTEAM_FAKE_DIR/n"
wteam verdict 01 fail --finding "problem $n"
"#,
    );
    let ev = dispatch_and_wait(&r, "01");
    assert_eq!(ev[0]["kind"], "blocked", "{ev:?}");
    let st = r.state();
    let t = &st["tickets"]["01"];
    assert_eq!(t["status"], "blocked");
    assert_eq!(t["rounds"], 3);
    let reason = t["blocked_reason"].as_str().unwrap();
    assert!(
        reason.contains("review failed 3 rounds") && reason.contains("problem 3"),
        "{reason}"
    );
    assert!(!reason.contains("problem 2"), "{reason}");
    let reviews = r.read(".wteam/demo/reviews/01.md");
    assert_eq!(reviews.matches("FAIL").count(), 3, "{reviews}");
    assert!(reviews.contains("problem 1") && reviews.contains("problem 3"));
    // The worker saw the findings of rounds 1 and 2, and the orchestrator heard of the block.
    let prompts = std::fs::read_to_string(r.fake_dir().join("worker-prompts")).unwrap();
    assert!(prompts.contains("problem 2"), "{prompts}");
    assert!(
        r.read(".wteam/demo/mail/orchestrator.1.md")
            .contains("problem 3")
    );

    let ready = r.wteam().args(["--json", "ready"]).assert().success();
    let ready = String::from_utf8_lossy(&ready.get_output().stdout).into_owned();
    assert!(
        ready.contains("\"02\"") && !ready.contains("\"01\""),
        "{ready}"
    );
}

#[test]
fn follow_up_needs_a_key() {
    let r = repo();
    r.ticket("01", "Add thing", "None", "src/01.txt", "");
    let out = r
        .wteam()
        .args(["verdict", "01", "pass", "--follow-up", "no key here"])
        .assert()
        .failure();
    let err = String::from_utf8_lossy(&out.get_output().stderr).into_owned();
    assert!(err.contains("key:text"), "{err}");
}

#[test]
fn pass_notes_reach_the_worker_mailbox() {
    let r = repo();
    r.ticket("01", "Add thing", "None", "src/01.txt", "");
    r.agent_script(
        "reviewer",
        "wteam verdict 01 pass --note 'rename the helper later'\n",
    );
    let ev = dispatch_and_wait(&r, "01");
    assert_eq!(ev[0]["kind"], "passed", "{ev:?}");
    let mail = r.read(".wteam/demo/mail/worker.1.md");
    assert!(mail.contains("rename the helper later"), "{mail}");
    assert!(
        r.read(".wteam/demo/reviews/01.md")
            .contains("rename the helper later")
    );
}

/// Pass 01..03, each with a `dup` follow-up; 01 also leaves a `once` one.
fn pass_three_with_follow_ups(r: &Repo) {
    for (id, name) in [("01", "One"), ("02", "Two"), ("03", "Three")] {
        r.ticket(id, name, "None", &format!("src/{id}.txt"), "");
    }
    r.agent_script(
        "reviewer",
        r#"
id=$(printf '%s\n' "$WTEAM_PROMPT" | sed -n 's/^Review request: ticket \([0-9]*\) .*/\1/p' | head -1)
if [ "$id" = 01 ]; then
  wteam verdict "$id" pass --follow-up "dup:shared helper for $id" --follow-up "once:docs typo"
else
  wteam verdict "$id" pass --follow-up "dup:shared helper for $id"
fi
"#,
    );
    for id in ["01", "02", "03"] {
        let ev = dispatch_and_wait(r, id);
        assert_eq!(ev[0]["kind"], "passed", "{ev:?}");
    }
}

#[test]
fn follow_ups_append_to_the_queue_and_count_per_key() {
    let r = repo();
    pass_three_with_follow_ups(&r);
    let queue = r.read(".wteam/demo/queue.md");
    assert!(
        queue.contains("- [dup] 01: shared helper for 01 (from reviewer.1)\n"),
        "{queue}"
    );
    assert!(queue.contains("- [dup] 03: shared helper for 03 (from reviewer.1)\n"));
    assert!(queue.contains("- [once] 01: docs typo (from reviewer.1)\n"));

    let out = r.wteam().args(["--json", "report"]).assert().success();
    let v: Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert_eq!(v["follow_ups"]["promote"][0]["key"], "dup");
    assert_eq!(v["follow_ups"]["promote"][0]["count"], 3);
    assert_eq!(v["follow_ups"]["watching"][0]["key"], "once");
    assert_eq!(v["follow_ups"]["watching"][0]["count"], 1);
}

#[test]
fn report_lists_done_blocked_follow_ups_and_next_step() {
    let r = repo();
    pass_three_with_follow_ups(&r);
    let sha = r
        .git(&["rev-parse", "--short=7", "HEAD"])
        .trim()
        .to_string();
    r.ticket("04", "Stuck thing", "None", "src/04.txt", "");
    r.agent_script("worker", "true\n");
    r.wteam().args(["dispatch", "04"]).assert().success();
    r.wteam()
        .args(["block", "04", "--reason", "needs the payments API"])
        .assert()
        .success();
    r.ticket("05", "Later thing", "None", "src/05.txt", "");

    let text = report_text(&r);
    for want in [
        "## Done",
        "03: Three",
        sha.as_str(),
        "## Blocked",
        "04: Stuck thing",
        "needs the payments API",
        "re-plan: needs the payments API",
        "## Follow-ups",
        "[dup] x3: promote to a ticket",
        "[once] x1: watching",
        "**Next step:** `wteam run`",
    ] {
        assert!(text.contains(want), "missing {want:?} in:\n{text}");
    }

    // Once nothing is runnable, the next step is to re-plan.
    std::fs::remove_file(r.effort().join("tickets/05-later-thing.md")).unwrap();
    let text = report_text(&r);
    assert!(
        text.contains("**Next step:** re-plan the blocked tickets, then `wteam pickup`"),
        "{text}"
    );

    let out = r.wteam().args(["--json", "report"]).assert().success();
    let v: Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert_eq!(v["done"].as_array().unwrap().len(), 3);
    assert_eq!(v["blocked"][0]["id"], "04");
    assert_eq!(v["blocked"][0]["action"], "re-plan: needs the payments API");
}

#[test]
fn cap_and_adapter_errors_get_their_recommended_actions() {
    let r = repo();
    r.ticket("01", "Capped", "None", "src/01.txt", "");
    r.ticket("02", "Crashed", "None", "src/02.txt", "");
    r.agent_script(
        "reviewer",
        "wteam verdict 01 fail --finding 'still wrong'\n",
    );
    dispatch_and_wait(&r, "01");
    r.agent_script("worker", "echo boom >&2\nexit 3\n");
    dispatch_and_wait(&r, "02");
    let text = report_text(&r);
    assert!(
        text.contains("review failed 3 rounds: still wrong")
            && text.contains("split the ticket or raise the worker model"),
        "{text}"
    );
    assert!(
        text.contains("check the CLI log at") && text.contains("worker.1-4.log"),
        "{text}"
    );
}

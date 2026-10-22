//! Self-healing turns, retries, liveness and token totals through the binary, with fake agents.

mod support;

use serde_json::Value;
use support::Repo;

fn repo() -> Repo {
    let r = Repo::new("demo");
    r.ticket("01", "Add thing", "None", "src/a.txt", "true");
    r.use_fake_agents();
    r
}

/// Dispatch ticket 01 with retry backoff disabled for every turn it starts.
fn dispatch(r: &Repo) {
    r.wteam()
        .env("WTEAM_BACKOFF_SCALE", "0")
        .args(["dispatch", "01"])
        .assert()
        .success();
}

/// Reviewer that passes ticket 01.
const PASS: &str = "wteam verdict 01 pass\n";

fn done(s: &Value) -> bool {
    s["tickets"]["01"]["status"] == "done" && s["agents"]["worker.1"]["busy"] == false
}

#[test]
fn silent_worker_is_reprompted_once_then_blocked() {
    let r = repo();
    r.agent_script("worker", "echo \"tokens: 1000\"\n");
    r.agent_script("reviewer", PASS);
    dispatch(&r);
    let s = r.wait_for(20, |s| {
        s["tickets"]["01"]["status"] == "blocked" && s["agents"]["worker.1"]["busy"] == false
    });
    assert_eq!(
        s["tickets"]["01"]["blocked_reason"],
        "worker ended without submitting"
    );
    assert_eq!(s["agents"]["worker.1"]["turns"], 2);
    let mail = r.read(".wteam/demo/mail/worker.1.md");
    assert!(
        mail.contains("You ended your turn without `wteam submit 01 --summary-file <path>` or `wteam block 01 --reason-file <path>`"),
        "{mail}"
    );
}

#[test]
fn silent_worker_that_submits_after_reprompt_passes() {
    let r = repo();
    r.agent_script(
        "worker",
        "case \"$WTEAM_PROMPT\" in\n\
         *\"ended your turn\"*) wteam submit 01 --summary done; echo \"tokens: 500\" ;;\n\
         *) echo \"tokens: 1000\" ;;\n\
         esac\n",
    );
    r.agent_script("reviewer", PASS);
    dispatch(&r);
    r.wait_for(20, done);
    assert_eq!(r.state()["tickets"]["01"]["status"], "done");

    // Token totals: per agent and across the team, both in JSON and text.
    let out = r.wteam().args(["--json", "status"]).assert().success();
    let v: Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert_eq!(v["tokens_total"], 1500, "{v}");
    let worker = v["agents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["name"] == "worker.1")
        .unwrap();
    assert_eq!(worker["tokens_total"], 1500, "{v}");
    let text = r.wteam().arg("status").assert().success();
    let text = String::from_utf8_lossy(&text.get_output().stdout).to_string();
    assert!(
        text.contains("worker.1") && text.contains("tokens=1500"),
        "{text}"
    );
    assert!(text.contains("total tokens=1500"), "{text}");
}

#[test]
fn killed_turn_is_reset_and_redelivered() {
    let r = repo();
    r.agent_script(
        "worker",
        "case \"$WTEAM_PROMPT\" in\n\
         *\"ended your turn\"*) wteam submit 01 --summary done ;;\n\
         esac\n",
    );
    r.agent_script("reviewer", PASS);
    // A worker whose turn process died mid-ticket: busy with a dead pid, ticket still doing.
    let mut child = std::process::Command::new("true").spawn().unwrap();
    let dead = child.id();
    child.wait().unwrap();
    r.write(
        ".wteam/demo/state.json",
        &format!(
            r#"{{"slug":"demo","tickets":{{"01":{{"status":"doing","assignee":"worker.1","pair":1}}}},
               "agents":{{"worker.1":{{"role":"worker","index":1,"busy":true,"turn_pid":{dead}}}}}}}"#
        ),
    );
    r.wteam().arg("status").assert().success();
    r.wait_for(20, done);
    let log = r.read(".wteam/demo/logs/worker.1-process.log");
    assert!(log.contains("busy reset"), "{log}");
}

#[test]
fn turn_pid_is_recorded_at_spawn() {
    let r = repo();
    r.agent_script("worker", "sleep 2\nwteam submit 01 --summary done\n");
    r.agent_script("reviewer", PASS);
    dispatch(&r);
    // The pid is recorded before dispatch returns, while the turn is still running.
    assert!(
        r.state()["agents"]["worker.1"]["turn_pid"].is_u64(),
        "{}",
        r.state()
    );
    r.wait_for(20, done);
}

#[test]
fn transient_failure_is_retried_then_succeeds() {
    let r = repo();
    r.agent_script(
        "worker",
        "echo attempt >> \"$WTEAM_FAKE_DIR/attempts\"\n\
         if [ \"$(grep -c . \"$WTEAM_FAKE_DIR/attempts\")\" -le 1 ]; then\n\
           echo 'API Error: 429 rate limit exceeded' >&2\n\
           exit 1\n\
         fi\n\
         wteam submit 01 --summary ok\n",
    );
    r.agent_script("reviewer", PASS);
    dispatch(&r);
    r.wait_for(20, done);
    assert_eq!(r.read(".wteam-fake/attempts").lines().count(), 2);
    assert_eq!(r.state()["agents"]["worker.1"]["turns"], 1);
}

#[test]
fn transient_failure_blocks_after_three_retries() {
    let r = repo();
    r.agent_script(
        "worker",
        "echo attempt >> \"$WTEAM_FAKE_DIR/attempts\"\n\
         echo 'upstream overloaded' >&2\n\
         exit 1\n",
    );
    r.agent_script("reviewer", PASS);
    dispatch(&r);
    let s = r.wait_for(20, |s| s["tickets"]["01"]["status"] == "blocked");
    assert_eq!(r.read(".wteam-fake/attempts").lines().count(), 4);
    let reason = s["tickets"]["01"]["blocked_reason"].as_str().unwrap();
    assert!(reason.starts_with("worker.1 turn failed"), "{reason}");
}

#[test]
fn architecture_contract_is_referenced_in_brief_and_review() {
    let r = repo();
    r.write(".wteam/demo/architecture.md", "# contract\n");
    r.agent_script(
        "worker",
        "printf '%s' \"$WTEAM_PROMPT\" > \"$WTEAM_FAKE_DIR/worker-brief\"\n\
         mkdir -p src\necho edit >> src/a.txt\n\
         wteam submit 01 --summary s\n",
    );
    r.agent_script(
        "reviewer",
        "printf '%s' \"$WTEAM_PROMPT\" > \"$WTEAM_FAKE_DIR/review-request\"\nwteam verdict 01 pass\n",
    );
    dispatch(&r);
    r.wait_for(20, done);
    let brief = r.read(".wteam-fake/worker-brief");
    assert!(brief.contains(".wteam/demo/architecture.md"), "{brief}");
    let req = r.read(".wteam-fake/review-request");
    assert!(req.contains(".wteam/demo/architecture.md"), "{req}");
}

#[test]
fn deliver_clears_a_stale_pid_so_heal_does_not_respawn() {
    let r = repo();
    r.agent_script("worker", "wteam submit 01 --summary done\n");
    r.agent_script("reviewer", PASS);
    // An idle worker whose recorded pid belongs to a dead turn.
    let mut child = std::process::Command::new("true").spawn().unwrap();
    let dead = child.id();
    child.wait().unwrap();
    r.write(
        ".wteam/demo/state.json",
        &format!(
            r#"{{"slug":"demo","agents":{{"worker.1":{{"role":"worker","index":1,"busy":false,"turn_pid":{dead}}}}}}}"#
        ),
    );
    dispatch(&r);
    r.wait_for(20, done);
    // The new turn's own process must not see the stale pid and reset busy.
    let log = r.read(".wteam/demo/logs/worker.1-process.log");
    assert!(!log.contains("busy reset"), "{log}");
}

#[test]
fn untracked_file_is_in_review_diff_and_commit() {
    let r = Repo::new("demo");
    r.ticket("01", "Add thing", "None", "src/a.txt, src/new.txt", "true");
    r.use_fake_agents();
    r.agent_script(
        "worker",
        "mkdir -p src\necho fresh > src/new.txt\necho edit >> src/a.txt\n\
         wteam submit 01 --summary s\n",
    );
    r.agent_script(
        "reviewer",
        "printf '%s' \"$WTEAM_PROMPT\" > \"$WTEAM_FAKE_DIR/review-request\"\nwteam verdict 01 pass\n",
    );
    dispatch(&r);
    r.wait_for(20, done);
    let req = r.read(".wteam-fake/review-request");
    assert!(req.contains("+fresh"), "{req}");
    let files = r.git(&["show", "--name-only", "--format=", "HEAD"]);
    assert!(files.lines().any(|l| l == "src/new.txt"), "{files}");
}

#[test]
fn silent_reviewer_is_reprompted_then_blocked() {
    let r = repo();
    r.agent_script("worker", "wteam submit 01 --summary done\n");
    r.agent_script("reviewer", "echo \"tokens: 10\"\n");
    dispatch(&r);
    let s = r.wait_for(20, |s| {
        s["tickets"]["01"]["status"] == "blocked" && s["agents"]["reviewer.1"]["busy"] == false
    });
    assert_eq!(
        s["tickets"]["01"]["blocked_reason"],
        "reviewer ended without a verdict"
    );
    assert_eq!(s["agents"]["reviewer.1"]["turns"], 2);
    let mail = r.read(".wteam/demo/mail/reviewer.1.md");
    assert!(mail.contains("wteam verdict 01 pass|fail"), "{mail}");
}

#[test]
fn ticket_dispatched_during_busy_turn_runs_after_it_ends() {
    let r = repo();
    r.ticket("02", "Other thing", "None", "src/b.txt", "true");
    r.agent_script(
        "worker",
        "case \"$WTEAM_PROMPT\" in\n\
         *\"(ticket 01)\"*) wteam submit 01 --summary a; sleep 3 ;;\n\
         *\"(ticket 02)\"*) wteam submit 02 --summary b ;;\n\
         esac\n",
    );
    r.agent_script("reviewer", "wteam verdict 01 pass; wteam verdict 02 pass\n");
    dispatch(&r);
    r.wait_for(20, |s| {
        s["tickets"]["01"]["status"] == "done" && s["agents"]["worker.1"]["busy"] == true
    });
    r.wteam()
        .env("WTEAM_BACKOFF_SCALE", "0")
        .args(["dispatch", "02"])
        .assert()
        .success();
    let s = r.wait_for(20, |s| {
        s["tickets"]["02"]["status"] == "done" && s["agents"]["worker.1"]["busy"] == false
    });
    assert_eq!(s["tickets"]["02"]["status"], "done", "{s:#}");
}

#[test]
fn turn_lifecycle_is_traced_in_order() {
    let r = repo();
    r.agent_script("worker", "wteam submit 01 --summary done\n");
    r.agent_script("reviewer", PASS);
    dispatch(&r);
    r.wait_for(20, done);
    // The turn process writes its last line after releasing busy, so poll for it.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let log = loop {
        let log = r.read(".wteam/demo/logs/worker.1-process.log");
        if log.contains("release idle") || std::time::Instant::now() >= deadline {
            break log;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    let at = |n: &str| log.find(n).unwrap_or_else(|| panic!("no `{n}` in:\n{log}"));
    // Order holds within each process: the parent's deliver lines, then the child's turn
    // lines. The parent's `spawned` and the child's `turn start` race, so they are not ordered.
    let parent = ["deliver claim stamp=", "deliver spawned pid="].map(at);
    let child = [
        "turn start turn=1 first=true",
        "turn end turn=1",
        "release idle",
    ]
    .map(at);
    assert!(parent[0] < parent[1], "deliver out of order:\n{log}");
    assert!(
        parent[0] < child[0],
        "turn started before its claim:\n{log}"
    );
    assert!(
        child.windows(2).all(|w| w[0] < w[1]),
        "turn out of order:\n{log}"
    );
    assert!(
        log.lines().all(|l| l.starts_with("20")),
        "untimestamped line:\n{log}"
    );
}

#[test]
fn run_revives_busy_agent_without_pid_within_grace() {
    let r = repo();
    r.agent_script("orchestrator", "echo ok\n");
    r.agent_script("worker", "wteam submit 01 --summary done\n");
    r.agent_script("reviewer", PASS);
    // Worker 1 holds ticket 01 and is busy with no pid: its spawn stamp is fresh, so only
    // the run's own polling can reset it.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    r.write(
        ".wteam/demo/state.json",
        &format!(
            r#"{{"slug":"demo","tickets":{{"01":{{"status":"doing","assignee":"worker.1","pair":1}}}},
               "agents":{{"worker.1":{{"role":"worker","index":1,"busy":true,"turn_spawned_at":{now}}}}}}}"#
        ),
    );
    let out = r
        .wteam()
        .env("WTEAM_BACKOFF_SCALE", "0")
        .args(["run", "--timeout", "120"])
        .assert()
        .success();
    let text = String::from_utf8_lossy(&out.get_output().stdout).to_string();
    assert!(!text.contains("timed out"), "{text}");
    let s = r.state();
    assert_eq!(s["tickets"]["01"]["status"], "done", "{s:#}");
    let log = r.read(".wteam/demo/logs/worker.1-process.log");
    assert!(log.contains("busy reset"), "{log}");
}

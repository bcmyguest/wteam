//! Team-graph routing: command gates, `**Verify:**` gates, multi-reviewer paths,
//! fan-in, and human approval gates, driven through the binary with fake agents.

mod support;

use predicates::str::contains;
use support::Repo;

/// Worker that edits its ticket's file and submits, parsing the ticket id from its brief or failure mail.
const WORKER: &str = r#"
id=$(printf '%s\n' "$WTEAM_PROMPT" | sed -n 's/.*[Tt]icket \([0-9][0-9]\).*/\1/p' | head -1)
printf '%s\n' "$WTEAM_PROMPT" >> "$WTEAM_FAKE_DIR/worker-prompts"
mkdir -p src
echo edit >> "src/$id.txt"
wteam submit "$id" --summary "edited $id"
"#;

/// Reviewer that passes the review request it was sent.
const REVIEWER_PASS: &str = r#"
for id in $(printf '%s\n' "$WTEAM_PROMPT" | sed -n 's/^Review request: ticket \([0-9][0-9]\).*/\1/p'); do
  echo "$WTEAM_AGENT" >> "$WTEAM_FAKE_DIR/reviewer-runs"
  wteam verdict "$id" pass
done
"#;

/// Team: worker -> tests (command) -> reviewer -> worker on fail.
fn gated_team(script: &str) -> String {
    format!(
        r#"digraph gated {{
  graph [max_visits=3]
  orchestrator [agent="orchestrator"]
  worker       [agent="worker", count="$pairs"]
  tests        [shape=parallelogram, script="{script}"]
  reviewer     [agent="reviewer", count="$pairs"]
  orchestrator -> worker   [on=dispatch, kind=broadcast]
  worker -> tests          [on=submit]
  tests -> reviewer        [kind=pair]
  reviewer -> worker       [on=fail, kind=pair]
  reviewer -> orchestrator [on=pass, kind=fan-in]
  worker -> orchestrator   [on=blocked, kind=fan-in]
}}
"#
    )
}

/// Team mirroring `presets/security.dot`, with the `cargo test` gate replaced by `true`.
const SECURITY_TEAM: &str = r#"digraph security {
  graph [max_visits=3]
  orchestrator [agent="orchestrator"]
  worker       [agent="worker", count="$pairs"]
  tests        [shape=parallelogram, script="true"]
  reviewer     [agent="reviewer", count="$pairs"]
  security     [agent="reviewer", count="$pairs"]
  orchestrator -> worker   [on=dispatch, kind=broadcast]
  worker -> tests          [on=submit]
  tests -> reviewer        [kind=pair]
  reviewer -> security     [on=pass, kind=pair]
  reviewer -> worker       [on=fail, kind=pair]
  security -> worker       [on=fail, kind=pair]
  security -> orchestrator [on=pass, kind=fan-in]
  worker -> orchestrator   [on=blocked, kind=fan-in]
}
"#;

/// Team with a human approval gate after the reviewer's PASS.
const HUMAN_TEAM: &str = r#"digraph human {
  graph [max_visits=3]
  orchestrator [agent="orchestrator"]
  worker       [agent="worker", count="$pairs"]
  reviewer     [agent="reviewer", count="$pairs"]
  boss         [shape=hexagon]
  orchestrator -> worker   [on=dispatch, kind=broadcast]
  worker -> reviewer       [on=submit, kind=pair]
  reviewer -> worker       [on=fail, kind=pair]
  reviewer -> boss         [on=pass]
  worker -> orchestrator   [on=blocked, kind=fan-in]
}
"#;

/// A repo with fake agents, a project `.wteam/team.dot` when given, and idle orchestrator/planner.
fn repo(team: Option<&str>) -> Repo {
    let r = Repo::new("demo");
    r.use_fake_agents();
    r.agent_script("orchestrator", "echo ok\n");
    r.agent_script("planner", "echo ok\n");
    if let Some(team) = team {
        r.write(".wteam/team.dot", team);
    }
    r
}

fn reviewer_never_woke(r: &Repo) -> bool {
    r.read(".wteam-fake/reviewer-runs").is_empty()
}

#[test]
fn failing_command_gate_never_wakes_reviewer_and_counts_rounds() {
    let r = repo(Some(&gated_team("echo broken-check; exit 1")));
    r.ticket("01", "Add thing", "None", "src/01.txt", "");
    r.agent_script("worker", WORKER);
    r.agent_script("reviewer", REVIEWER_PASS);
    r.wteam().args(["dispatch", "01"]).assert().success();

    let s = r.wait_for(60, |s| s["tickets"]["01"]["status"] == "blocked");

    assert!(reviewer_never_woke(&r), "reviewer ran on a failed gate");
    assert_eq!(s["tickets"]["01"]["rounds"], 3);
    let reason = s["tickets"]["01"]["blocked_reason"].as_str().unwrap();
    assert!(reason.contains("broken-check"), "{reason}");
    assert!(r.read(".wteam/demo/reviews/01.md").contains("FAIL (tests)"));
    let prompts = r.read(".wteam-fake/worker-prompts");
    assert!(
        prompts.contains("broken-check"),
        "worker got no output tail"
    );
}

#[test]
fn verify_failure_acts_as_a_command_gate() {
    let r = repo(None);
    r.ticket(
        "01",
        "Add thing",
        "None",
        "src/01.txt",
        "echo nope && false",
    );
    r.agent_script("worker", WORKER);
    r.agent_script("reviewer", REVIEWER_PASS);
    r.wteam().args(["dispatch", "01"]).assert().success();

    let s = r.wait_for(60, |s| s["tickets"]["01"]["status"] == "blocked");

    assert!(reviewer_never_woke(&r), "reviewer ran on a failed verify");
    assert_eq!(s["tickets"]["01"]["rounds"], 3);
    let reason = s["tickets"]["01"]["blocked_reason"].as_str().unwrap();
    assert!(reason.contains("nope"), "{reason}");
}

#[test]
fn security_path_needs_both_reviewers_to_pass() {
    let r = repo(Some(SECURITY_TEAM));
    r.ticket("01", "Add thing", "None", "src/01.txt", "");
    r.agent_script("worker", WORKER);
    r.agent_script("reviewer", REVIEWER_PASS);
    // Security rejects the first round, then passes.
    r.agent_script(
        "security",
        r#"
id=$(printf '%s\n' "$WTEAM_PROMPT" | sed -n 's/^Review request: ticket \([0-9][0-9]\).*/\1/p')
n=$(cat "$WTEAM_FAKE_DIR/sec" 2>/dev/null || echo 0)
n=$((n + 1))
echo $n > "$WTEAM_FAKE_DIR/sec"
if [ $n -eq 1 ]; then
  wteam verdict "$id" fail --finding "sec issue"
else
  wteam verdict "$id" pass
fi
"#,
    );
    r.wteam().args(["dispatch", "01"]).assert().success();

    r.wait_for(60, |s| s["tickets"]["01"]["status"] == "done");

    assert_eq!(r.read(".wteam-fake/reviewer-runs").lines().count(), 2);
    let reviews = r.read(".wteam/demo/reviews/01.md");
    assert!(reviews.contains("FAIL (security.1)"), "{reviews}");
    assert!(reviews.contains("PASS (security.1)"), "{reviews}");
    assert!(r.git(&["log", "--oneline"]).contains("(wteam 01)"));
}

#[test]
fn one_pass_holds_the_commit_until_the_next_reviewer_passes() {
    let r = repo(Some(SECURITY_TEAM));
    r.ticket("01", "Add thing", "None", "src/01.txt", "");
    r.agent_script("worker", WORKER);
    r.agent_script("reviewer", REVIEWER_PASS);
    // Security takes the review and never answers.
    r.agent_script(
        "security",
        "echo \"$WTEAM_AGENT\" >> \"$WTEAM_FAKE_DIR/security-runs\"\n",
    );
    r.wteam().args(["dispatch", "01"]).assert().success();

    let s = r.wait_for(60, |s| {
        s["agents"]["security.1"]["turns"].as_u64().unwrap_or(0) >= 1
    });

    assert_ne!(s["tickets"]["01"]["status"], "done");
    assert_eq!(
        s["tickets"]["01"]["passes"],
        serde_json::json!(["reviewer"])
    );
    assert!(!r.git(&["log", "--oneline"]).contains("(wteam 01)"));
}

#[test]
fn fan_in_sends_every_submit_to_reviewer_1() {
    let r = repo(None);
    r.write(
        ".wteam/demo/state.json",
        r#"{"slug":"demo","run":{"pairs":2,"isolation":"none","transport":"resume","team":"fan-in"}}"#,
    );
    r.ticket("01", "First thing", "None", "src/01.txt", "");
    r.ticket("02", "Second thing", "None", "src/02.txt", "");
    r.agent_script("worker", WORKER);
    r.agent_script("reviewer", REVIEWER_PASS);
    r.wteam()
        .args(["dispatch", "01", "--pair", "1"])
        .assert()
        .success();
    r.wteam()
        .args(["dispatch", "02", "--pair", "2"])
        .assert()
        .success();

    r.wait_for(60, |s| {
        s["tickets"]["01"]["status"] == "done" && s["tickets"]["02"]["status"] == "done"
    });

    let seen = r.read(".wteam-fake/reviewer-runs");
    assert_eq!(
        seen.lines().collect::<Vec<_>>(),
        ["reviewer.1", "reviewer.1"]
    );
}

#[test]
fn human_gate_holds_pass_until_approved() {
    let r = repo(Some(HUMAN_TEAM));
    r.ticket("01", "Add thing", "None", "src/01.txt", "");
    r.agent_script("worker", WORKER);
    r.agent_script("reviewer", REVIEWER_PASS);

    r.wteam().args(["approve", "01"]).assert().failure();

    r.wteam().args(["dispatch", "01"]).assert().success();
    let s = r.wait_for(60, |s| s["tickets"]["01"]["status"] == "awaiting_approval");
    assert!(
        s["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["kind"] == "needs_approval"),
        "{s:#}"
    );
    assert!(!r.git(&["log", "--oneline"]).contains("(wteam 01)"));

    r.wteam().args(["approve", "01"]).assert().success();
    r.wait_for(30, |s| s["tickets"]["01"]["status"] == "done");
    assert!(r.git(&["log", "--oneline"]).contains("(wteam 01)"));

    r.wteam().args(["approve", "01"]).assert().failure();
}

#[test]
fn failing_command_after_pass_counts_a_round_and_returns_to_worker() {
    let r = repo(Some(
        r#"digraph post {
  graph [max_visits=3]
  orchestrator [agent="orchestrator"]
  worker       [agent="worker", count="$pairs"]
  reviewer     [agent="reviewer", count="$pairs"]
  lint         [shape=parallelogram, script="echo lint-broke; exit 1"]
  orchestrator -> worker   [on=dispatch, kind=broadcast]
  worker -> reviewer       [on=submit, kind=pair]
  reviewer -> worker       [on=fail, kind=pair]
  reviewer -> lint         [on=pass]
  lint -> orchestrator     [on=pass, kind=fan-in]
  worker -> orchestrator   [on=blocked, kind=fan-in]
}
"#,
    ));
    r.ticket("01", "Add thing", "None", "src/01.txt", "");
    r.agent_script("worker", WORKER);
    r.agent_script("reviewer", REVIEWER_PASS);
    r.wteam().args(["dispatch", "01"]).assert().success();

    let s = r.wait_for(60, |s| s["tickets"]["01"]["status"] == "blocked");

    assert_eq!(s["tickets"]["01"]["rounds"], 3);
    assert_ne!(s["tickets"]["01"]["status"], "done");
    let reason = s["tickets"]["01"]["blocked_reason"].as_str().unwrap();
    assert!(reason.contains("lint-broke"), "{reason}");
    assert!(r.read(".wteam-fake/worker-prompts").contains("lint-broke"));
    assert!(!r.git(&["log", "--oneline"]).contains("(wteam 01)"));
}

/// Team whose second reviewer `security` has no outgoing links.
const ROUTELESS_TEAM: &str = r#"digraph routeless {
  graph [max_visits=3]
  orchestrator [agent="orchestrator"]
  worker       [agent="worker", count="$pairs"]
  reviewer     [agent="reviewer", count="$pairs"]
  security     [agent="reviewer", count="$pairs"]
  orchestrator -> worker   [on=dispatch, kind=broadcast]
  worker -> reviewer       [on=submit, kind=pair]
  reviewer -> worker       [on=fail, kind=pair]
  reviewer -> security     [on=pass, kind=pair]
  worker -> orchestrator   [on=blocked, kind=fan-in]
}
"#;

#[test]
fn route_less_second_reviewer_reviews_before_commit() {
    let r = repo(Some(ROUTELESS_TEAM));
    r.ticket("01", "Add thing", "None", "src/01.txt", "");
    r.agent_script("worker", WORKER);
    r.agent_script("reviewer", REVIEWER_PASS);
    r.agent_script(
        "security",
        r#"
id=$(printf '%s\n' "$WTEAM_PROMPT" | sed -n 's/^Review request: ticket \([0-9][0-9]\).*/\1/p')
echo "$WTEAM_AGENT" >> "$WTEAM_FAKE_DIR/security-runs"
wteam verdict "$id" pass
"#,
    );
    r.wteam().args(["dispatch", "01"]).assert().success();

    r.wait_for(60, |s| s["tickets"]["01"]["status"] == "done");

    assert_eq!(r.read(".wteam-fake/security-runs").trim(), "security.1");
    assert!(
        r.read(".wteam/demo/reviews/01.md")
            .contains("PASS (security.1)")
    );
}

#[test]
fn commit_refused_until_every_reviewer_on_the_chain_has_passed() {
    let r = repo(Some(ROUTELESS_TEAM));
    r.ticket("01", "Add thing", "None", "src/01.txt", "");
    r.write(
        ".wteam/demo/state.json",
        r#"{"slug":"demo","tickets":{"01":{"status":"review","assignee":"security.1","pair":1}}}"#,
    );

    // The chain is reviewer -> security, but `reviewer` never passed this round.
    r.wteam()
        .env("WTEAM_AGENT", "security.1")
        .args(["verdict", "01", "pass"])
        .assert()
        .failure()
        .stderr(contains("cannot commit"));

    let s = r.state();
    assert_eq!(s["tickets"]["01"]["status"], "review");
    assert!(!r.git(&["log", "--oneline"]).contains("(wteam 01)"));
}

#[test]
fn fail_clears_passes_so_the_next_round_needs_every_reviewer_again() {
    let r = repo(Some(SECURITY_TEAM));
    r.ticket("01", "Add thing", "None", "src/01.txt", "");
    // Round 1 submits; later prompts (the failure mail) stay silent so round 2 never starts.
    r.agent_script(
        "worker",
        r#"
case "$WTEAM_PROMPT" in
  *"failed (round"*) exit 0 ;;
  *) id=01; mkdir -p src; echo edit >> "src/$id.txt"; wteam submit "$id" --summary "edited" ;;
esac
"#,
    );
    r.agent_script("reviewer", REVIEWER_PASS);
    r.agent_script(
        "security",
        "wteam verdict 01 fail --finding \"sec issue\"\n",
    );
    r.wteam().args(["dispatch", "01"]).assert().success();

    let s = r.wait_for(60, |s| s["tickets"]["01"]["rounds"] == 1);

    assert_eq!(s["tickets"]["01"]["passes"], serde_json::json!([]));
}

#[test]
fn command_gate_timeout_kills_the_check_and_counts_a_round() {
    let r = repo(
        Some(&gated_team("sleep 5"))
            .map(|t| t.replace("script=\"sleep 5\"", "script=\"sleep 5\", timeout=1"))
            .as_deref(),
    );
    r.ticket("01", "Add thing", "None", "src/01.txt", "");
    r.agent_script("worker", WORKER);
    r.agent_script("reviewer", REVIEWER_PASS);
    r.wteam().args(["dispatch", "01"]).assert().success();

    let s = r.wait_for(60, |s| s["tickets"]["01"]["status"] == "blocked");

    assert!(reviewer_never_woke(&r), "reviewer ran on a timed-out gate");
    assert_eq!(s["tickets"]["01"]["rounds"], 3);
    let reason = s["tickets"]["01"]["blocked_reason"].as_str().unwrap();
    assert!(reason.contains("timed out after 1s"), "{reason}");
}

#[test]
fn pass_cycle_blocks_instead_of_looping() {
    let r = repo(Some(
        r#"digraph cycle {
  graph [max_visits=3]
  orchestrator [agent="orchestrator"]
  worker       [agent="worker", count="$pairs"]
  reviewer     [agent="reviewer", count="$pairs"]
  security     [agent="reviewer", count="$pairs"]
  orchestrator -> worker   [on=dispatch, kind=broadcast]
  worker -> reviewer       [on=submit, kind=pair]
  reviewer -> worker       [on=fail, kind=pair]
  reviewer -> security     [on=pass, kind=pair]
  security -> reviewer     [on=pass, kind=pair]
  security -> worker       [on=fail, kind=pair]
  worker -> orchestrator   [on=blocked, kind=fan-in]
}
"#,
    ));
    r.ticket("01", "Add thing", "None", "src/01.txt", "");
    r.agent_script("worker", WORKER);
    r.agent_script("reviewer", REVIEWER_PASS);
    r.agent_script("security", "wteam verdict 01 pass\n");
    r.wteam().args(["dispatch", "01"]).assert().success();

    let s = r.wait_for(60, |s| s["tickets"]["01"]["status"] == "blocked");

    assert_eq!(
        s["tickets"]["01"]["blocked_reason"],
        "pass cycle at reviewer"
    );
    assert_eq!(r.read(".wteam-fake/reviewer-runs").lines().count(), 1);
}

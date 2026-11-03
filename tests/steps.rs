//! Multi-step teams: a PASS that reaches another worker node hands the ticket to that step.

mod support;

use support::Repo;

fn team(run_attrs: &str, gate: bool) -> String {
    let middle = if gate {
        "  approval [shape=hexagon]\n  plan_rev -> approval [on=pass]\n  approval -> run [on=pass]\n"
    } else {
        "  plan_rev -> run [on=pass, kind=pair]\n"
    };
    format!(
        r#"digraph steps {{
  planner      [agent="planner"]
  orchestrator [agent="orchestrator"]
  plan     [agent="worker", skill="plan-experiment"]
  plan_rev [agent="reviewer"]
  run      [agent="worker", skill="run-experiment"{run_attrs}]
  run_rev  [agent="reviewer", skill="sanity-check-numbers"]
  orchestrator -> plan [on=dispatch, kind=broadcast]
  plan -> plan_rev     [on=submit, kind=pair]
  plan_rev -> plan     [on=fail, kind=pair]
{middle}  run -> run_rev       [on=submit, kind=pair]
  run_rev -> run       [on=fail, kind=pair]
  run_rev -> orchestrator [on=pass, kind=fan-in]
  plan -> orchestrator [on=blocked, kind=fan-in]
  run -> orchestrator  [on=blocked, kind=fan-in]
}}
"#
    )
}

/// Each step records `agent|session|prompt` and edits its own file. The run reviewer FAILs once
/// when `fail-run` exists.
fn repo(team_dot: &str) -> Repo {
    let r = Repo::new("demo");
    r.ticket(
        "01",
        "Add thing",
        "None",
        "src/plan.txt, src/run.txt",
        "true",
    );
    r.use_fake_agents();
    r.write(".wteam/team.dot", team_dot);
    for step in ["plan", "run"] {
        r.agent_script(
            step,
            &format!(
                r#"
printf '%s|%s|%s\n---\n' "$WTEAM_AGENT" "$WTEAM_SESSION" "$WTEAM_PROMPT" >> "$WTEAM_FAKE_DIR/turns"
if [ -e "$WTEAM_FAKE_DIR/block-{step}" ] && [ ! -e "$WTEAM_FAKE_DIR/blocked-{step}" ]; then
  touch "$WTEAM_FAKE_DIR/blocked-{step}"
  wteam block 01 --reason "decisions needed" || exit 1
  exit 0
fi
mkdir -p src && echo {step} >> src/{step}.txt
wteam submit 01 --summary {step} || exit 1
echo "session: S-$WTEAM_AGENT"
echo "tokens: 1000"
"#
            ),
        );
    }
    r.agent_script("plan_rev", "wteam verdict 01 pass\n");
    r.agent_script(
        "run_rev",
        r#"
printf '%s\n---\n' "$WTEAM_PROMPT" >> "$WTEAM_FAKE_DIR/run-reviews"
if [ -e "$WTEAM_FAKE_DIR/fail-run" ] && [ ! -e "$WTEAM_FAKE_DIR/failed" ]; then
  touch "$WTEAM_FAKE_DIR/failed"
  wteam verdict 01 fail --finding "recheck the numbers" || exit 1
else
  wteam verdict 01 pass || exit 1
fi
"#,
    );
    r
}

fn turns(r: &Repo) -> Vec<String> {
    std::fs::read_to_string(r.fake_dir().join("turns"))
        .unwrap_or_default()
        .split("\n---\n")
        .filter(|t| !t.trim().is_empty())
        .map(str::to_string)
        .collect()
}

fn run_to_done(r: &Repo) {
    r.wteam().args(["dispatch", "01"]).assert().success();
    r.wait_for(30, |s| s["tickets"]["01"]["status"] == "done");
}

#[test]
fn a_pass_hands_the_ticket_to_the_next_step_and_commits_once_at_the_end() {
    let r = repo(&team(", continue=true", false));
    let before = r.git(&["rev-list", "--count", "HEAD"]);
    run_to_done(&r);

    let t = turns(&r);
    assert_eq!(t.len(), 2, "{t:?}");
    assert!(t[0].starts_with("plan.1||"), "{}", t[0]);
    assert!(t[0].contains("Use the `plan-experiment` skill"), "{}", t[0]);
    // continue=true: the run step resumes the plan step's session.
    assert!(t[1].starts_with("run.1|S-plan.1|"), "{}", t[1]);
    assert!(
        t[1].contains("Step `run`: step `plan` passed review"),
        "{}",
        t[1]
    );
    assert!(t[1].contains("Use the `run-experiment` skill"), "{}", t[1]);
    let review = std::fs::read_to_string(r.fake_dir().join("run-reviews")).unwrap();
    assert!(
        review.contains("Use the `sanity-check-numbers` skill"),
        "{review}"
    );

    let after = r.git(&["rev-list", "--count", "HEAD"]);
    assert_eq!(
        after.trim().parse::<u32>().unwrap(),
        before.trim().parse::<u32>().unwrap() + 1
    );
    let files = r.git(&["show", "--name-only", "--format=", "HEAD"]);
    assert!(
        files.contains("src/plan.txt") && files.contains("src/run.txt"),
        "{files}"
    );
    assert!(r.state()["tickets"]["01"]["step"].is_null());
}

#[test]
fn without_continue_the_next_step_starts_a_fresh_session() {
    let r = repo(&team("", false));
    run_to_done(&r);
    let t = turns(&r);
    assert!(t[1].starts_with("run.1||"), "{}", t[1]);
}

#[test]
fn a_fail_in_a_later_step_returns_to_that_steps_worker() {
    let r = repo(&team(", continue=true", false));
    r.write(".wteam-fake/fail-run", "");
    run_to_done(&r);
    let t = turns(&r);
    let agents: Vec<&str> = t.iter().map(|t| t.split('|').next().unwrap()).collect();
    assert_eq!(agents, ["plan.1", "run.1", "run.1"], "{t:?}");
    assert!(t[2].contains("recheck the numbers"), "{}", t[2]);
    assert!(t[2].contains("Use the `run-experiment` skill"), "{}", t[2]);
}

#[test]
fn a_human_gate_between_steps_holds_the_ticket_until_approved() {
    let r = repo(&team("", true));
    r.wteam().args(["dispatch", "01"]).assert().success();
    r.wait_for(30, |s| s["tickets"]["01"]["status"] == "awaiting_approval");
    assert_eq!(turns(&r).len(), 1);
    r.wteam().args(["approve", "01"]).assert().success();
    r.wait_for(30, |s| s["tickets"]["01"]["status"] == "done");
    assert_eq!(turns(&r).len(), 2);
}

#[test]
fn after_a_human_gate_the_next_step_stays_on_the_tickets_pair() {
    let two = team("", true)
        .replace("[agent=\"worker\"", "[agent=\"worker\", count=2")
        .replace("[agent=\"reviewer\"", "[agent=\"reviewer\", count=2");
    let r = repo(&two);
    r.wteam()
        .args(["dispatch", "01", "--pair", "2"])
        .assert()
        .success();
    r.wait_for(30, |s| s["tickets"]["01"]["status"] == "awaiting_approval");
    r.wteam().args(["approve", "01"]).assert().success();
    r.wait_for(30, |s| s["tickets"]["01"]["status"] == "done");
    let agents: Vec<String> = turns(&r)
        .iter()
        .map(|t| t.split('|').next().unwrap().to_string())
        .collect();
    assert_eq!(agents, ["plan.2", "run.2"]);
}

fn agents(r: &Repo) -> Vec<String> {
    turns(r)
        .iter()
        .map(|t| t.split('|').next().unwrap().to_string())
        .collect()
}

#[test]
fn a_requeued_ticket_resumes_at_its_step_with_the_note() {
    let r = repo(&team("", false));
    r.write(".wteam-fake/block-run", "");
    r.wteam().args(["dispatch", "01"]).assert().success();
    r.wait_for(30, |s| s["tickets"]["01"]["status"] == "blocked");
    r.wteam()
        .args(["requeue", "01", "--note", "1a 2c"])
        .assert()
        .success();
    run_to_done(&r);
    assert_eq!(agents(&r), ["plan.1", "run.1", "run.1"]);
    let t = turns(&r);
    assert!(t[2].contains("1a 2c"), "{}", t[2]);
    assert!(t[2].contains("Use the `run-experiment` skill"), "{}", t[2]);
}

#[test]
fn a_fresh_requeue_starts_again_at_the_first_step() {
    let r = repo(&team("", false));
    r.write(".wteam-fake/block-run", "");
    r.wteam().args(["dispatch", "01"]).assert().success();
    r.wait_for(30, |s| s["tickets"]["01"]["status"] == "blocked");
    r.wteam()
        .args(["requeue", "01", "--fresh"])
        .assert()
        .success();
    run_to_done(&r);
    assert_eq!(agents(&r), ["plan.1", "run.1", "plan.1", "run.1"]);
}

#[test]
fn every_step_needs_the_first_steps_count() {
    let r = Repo::new("demo");
    r.write(".wteam/team.dot", &team(", count=2", false));
    r.wteam()
        .args(["teams"])
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "step `run` has count 2; every worker step needs the count of `plan` (1)",
        ));
}

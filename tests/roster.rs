//! First-turn role prompts include the team roster.

mod support;

use support::Repo;

#[test]
fn worker_first_turn_gets_team_roster_in_role_prompt() {
    let r = Repo::new("demo");
    r.ticket("01", "Add thing", "None", "src/a.txt", "true");
    r.use_fake_agents();
    r.agent_script(
        "worker",
        r#"
echo "$WTEAM_ROLE_PROMPT" | grep -q '## Team' || exit 1
echo "$WTEAM_ROLE_PROMPT" | grep -q 'worker.1' || exit 1
echo "$WTEAM_ROLE_PROMPT" | grep -q 'reviewer.1' || exit 1
wteam submit 01 --summary ok || exit 1
"#,
    );
    r.agent_script("reviewer", "wteam verdict 01 pass\n");
    r.wteam().args(["dispatch", "01"]).assert().success();
    r.wteam()
        .args(["--json", "wait", "--timeout", "30"])
        .assert()
        .success();
}

#[test]
fn node_prompt_is_appended_to_the_role_prompt() {
    let r = Repo::new("demo");
    r.ticket("01", "Add thing", "None", "src/a.txt", "true");
    r.use_fake_agents();
    r.write("focus.md", "Audit the diff for overreach.\n");
    r.write(
        ".wteam/team.dot",
        r#"digraph focused {
  planner      [agent="planner"]
  orchestrator [agent="orchestrator"]
  worker       [agent="worker", prompt="Diagnose the failure first."]
  reviewer     [agent="reviewer", prompt="@focus.md"]
  orchestrator -> worker   [on=dispatch, kind=broadcast]
  worker -> reviewer       [on=submit, kind=pair]
  reviewer -> worker       [on=fail, kind=pair]
  reviewer -> orchestrator [on=pass, kind=fan-in]
  worker -> orchestrator   [on=blocked, kind=fan-in]
}
"#,
    );
    r.agent_script(
        "worker",
        r#"
echo "$WTEAM_ROLE_PROMPT" | grep -q '## Role focus' || exit 1
echo "$WTEAM_ROLE_PROMPT" | grep -q 'Diagnose the failure first.' || exit 1
echo "$WTEAM_ROLE_PROMPT" | grep -q '## Team' || exit 1
wteam submit 01 --summary ok || exit 1
"#,
    );
    r.agent_script(
        "reviewer",
        r#"
echo "$WTEAM_ROLE_PROMPT" | grep -q 'Audit the diff for overreach.' || exit 1
wteam verdict 01 pass
"#,
    );
    r.wteam().args(["dispatch", "01"]).assert().success();
    r.wteam()
        .args(["--json", "wait", "--timeout", "30"])
        .assert()
        .success()
        .stdout(predicates::str::contains("\"passed\""));
}

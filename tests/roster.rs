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

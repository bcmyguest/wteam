//! Ticket `Skill:` / `Review skill:` and node `skill=` reach every brief, resumed rounds included.

mod support;

use support::Repo;

const TEAM: &str = r#"digraph skilled {
  planner      [agent="planner"]
  orchestrator [agent="orchestrator"]
  worker       [agent="worker"]
  reviewer     [agent="reviewer", skill="audit-agent-diff"]
  orchestrator -> worker   [on=dispatch, kind=broadcast]
  worker -> reviewer       [on=submit, kind=pair]
  reviewer -> worker       [on=fail, kind=pair]
  reviewer -> orchestrator [on=pass, kind=fan-in]
  worker -> orchestrator   [on=blocked, kind=fan-in]
}
"#;

fn skilled_ticket(r: &Repo, skill: &str) {
    r.write(
        ".wteam/demo/tickets/01-add-thing.md",
        &format!(
            "# 01: Add thing\n\n**What to build:** Add thing.\n\n**Blocked by:** None\n\n**Status:** ready-for-agent\n\n**Files:** src/a.txt\n\n**Skill:** {skill}\n\n**Review skill:** sanity-check-numbers\n\n**Verify:** true\n\n- [ ] it works\n"
        ),
    );
}

#[test]
fn every_worker_and_reviewer_turn_names_its_skills() {
    let r = Repo::new("demo");
    r.use_fake_agents();
    r.write(".wteam/team.dot", TEAM);
    skilled_ticket(&r, "`diagnose-failure`");
    r.agent_script(
        "worker",
        r#"
printf '%s\n---\n' "$WTEAM_PROMPT" >> "$WTEAM_FAKE_DIR/worker-prompts"
mkdir -p src && echo edit >> src/a.txt
wteam submit 01 --summary ok || exit 1
echo "session: S1"
"#,
    );
    r.agent_script(
        "reviewer",
        r#"
printf '%s\n---\n' "$WTEAM_PROMPT" >> "$WTEAM_FAKE_DIR/reviewer-prompts"
if [ ! -e "$WTEAM_FAKE_DIR/failed" ]; then
  touch "$WTEAM_FAKE_DIR/failed"
  wteam verdict 01 fail --finding "again" || exit 1
else
  wteam verdict 01 pass || exit 1
fi
echo "session: R1"
"#,
    );
    r.wteam().args(["dispatch", "01"]).assert().success();
    r.wteam()
        .args(["--json", "wait", "--timeout", "30"])
        .assert()
        .success()
        .stdout(predicates::str::contains("\"passed\""));

    let read = |f: &str| std::fs::read_to_string(r.fake_dir().join(f)).unwrap();
    let worker = read("worker-prompts");
    let turns: Vec<&str> = worker.split("\n---\n").filter(|t| !t.is_empty()).collect();
    assert_eq!(turns.len(), 2, "{worker}");
    for t in &turns {
        assert!(
            t.contains("Use the `diagnose-failure` skill for this turn."),
            "{t}"
        );
    }
    let reviewer = read("reviewer-prompts");
    let turns: Vec<&str> = reviewer
        .split("\n---\n")
        .filter(|t| !t.is_empty())
        .collect();
    assert_eq!(turns.len(), 2, "{reviewer}");
    for t in &turns {
        assert!(
            t.contains(
                "Use the `audit-agent-diff` and `sanity-check-numbers` skills for this turn."
            ),
            "{t}"
        );
    }
}

#[test]
fn pickup_rejects_a_malformed_skill_name() {
    let r = Repo::new("demo");
    skilled_ticket(&r, "Diagnose the failure");
    r.wteam()
        .args(["pickup"])
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "ticket 01: skill `Diagnose the failure` is not a skill name",
        ));
}

#[test]
fn a_malformed_node_skill_fails_to_load() {
    let r = Repo::new("demo");
    r.write(
        ".wteam/team.dot",
        &TEAM.replace("audit-agent-diff", "Audit diffs"),
    );
    r.wteam()
        .args(["teams"])
        .assert()
        .success()
        .stdout(predicates::str::contains("is not a skill name"));
}

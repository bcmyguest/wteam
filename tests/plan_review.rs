//! Plan-review Goldfish chain (fake agents). Personas are ordinary agent defs.

mod support;

use support::Repo;

fn seed_plan(r: &Repo) {
    // Pickup treats untracked fake scripts as dirty; keep them out of git status.
    r.write(".gitignore", ".wteam-fake/\n.wteam/\n");
    r.git(&["add", ".gitignore"]);
    r.git(&["commit", "-q", "-m", "chore: ignore fake agents"]);
    r.write(".wteam/demo/spec.md", "# Goal\n\nShip a hello command.\n");
    r.ticket("01", "Tracer hello", "None", "src/hello.rs", "cargo test");
    r.use_fake_agents();
    for role in ["spec-comprehension", "spec-critic", "spec-readiness"] {
        r.write(
            &format!(".wteam/agents/{role}.toml"),
            &format!(
                "name = \"{role}\"\ncli = \"fake\"\nprompt = \"{role}\"\nprofile = \"plan\"\n"
            ),
        );
    }
}

fn pass_script() -> &'static str {
    r#"mkdir -p .wteam/demo/plan-reviews
printf 'FINDINGS:\n' > .wteam/demo/plan-reviews/v.md
wteam plan-verdict pass --file .wteam/demo/plan-reviews/v.md
"#
}

#[test]
fn plan_review_passes_when_every_goldfish_passes() {
    let r = Repo::new("demo");
    seed_plan(&r);
    for role in ["spec-comprehension", "spec-critic", "spec-readiness"] {
        r.agent_script(role, pass_script());
    }
    r.wteam().args(["pickup"]).assert().success();
    r.wteam()
        .args(["plan-review"])
        .assert()
        .success()
        .stdout(predicates::str::contains("PASSED"));
    assert!(
        r.path()
            .join(".wteam/demo/plan-reviews/PASSED.md")
            .is_file()
    );
}

#[test]
fn plan_review_stops_on_first_fail() {
    let r = Repo::new("demo");
    seed_plan(&r);
    r.agent_script(
        "spec-comprehension",
        r#"mkdir -p .wteam/demo/plan-reviews
printf 'FINDINGS:\n- ambiguous goal\n' > .wteam/demo/plan-reviews/v.md
wteam plan-verdict fail --file .wteam/demo/plan-reviews/v.md
"#,
    );
    for role in ["spec-critic", "spec-readiness"] {
        r.agent_script(role, pass_script());
    }
    r.wteam().args(["pickup"]).assert().success();
    r.wteam()
        .args(["plan-review"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("FAILED"))
        .stderr(predicates::str::contains("ambiguous goal"));
    assert!(
        !r.path()
            .join(".wteam/demo/plan-reviews/PASSED.md")
            .is_file()
    );
    assert!(
        r.path()
            .join(".wteam/demo/plan-reviews/01-spec-comprehension.md")
            .is_file()
    );
    assert!(
        !r.path()
            .join(".wteam/demo/plan-reviews/02-spec-critic.md")
            .is_file()
    );
}

#[test]
fn plan_review_persona_flag_selects_subset() {
    let r = Repo::new("demo");
    seed_plan(&r);
    r.agent_script("spec-readiness", pass_script());
    r.wteam().args(["pickup"]).assert().success();
    r.wteam()
        .args(["plan-review", "--persona", "spec-readiness"])
        .assert()
        .success()
        .stdout(predicates::str::contains("spec-readiness"));
}

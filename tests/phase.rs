//! Generic Goldfish phases (fake agents).

mod support;

use support::Repo;

fn seed(r: &Repo) {
    r.write(".gitignore", ".wteam-fake/\n.wteam/\n");
    r.git(&["add", ".gitignore"]);
    r.git(&["commit", "-q", "-m", "chore: ignore fake agents"]);
    r.write(".wteam/demo/spec.md", "# Goal\n\nShip a hello command.\n");
    r.ticket("01", "Tracer hello", "None", "src/hello.rs", "cargo test");
    r.use_fake_agents();
}

fn agent_toml(r: &Repo, role: &str, profile: &str) {
    r.write(
        &format!(".wteam/agents/{role}.toml"),
        &format!(
            "name = \"{role}\"\ncli = \"fake\"\nprompt = \"{role}\"\nprofile = \"{profile}\"\n"
        ),
    );
}

fn plan_pass_script() -> &'static str {
    r#"mkdir -p .wteam/demo/plan-reviews
printf 'FINDINGS:\n' > .wteam/demo/plan-reviews/v.md
wteam plan-verdict pass --file .wteam/demo/plan-reviews/v.md
"#
}

fn validate_pass_script() -> &'static str {
    r#"mkdir -p .wteam/demo/phases/validate
printf 'FINDINGS:\n' > .wteam/demo/phases/validate/v.md
wteam phase-verdict pass --file .wteam/demo/phases/validate/v.md
"#
}

#[test]
fn phase_plan_review_alias_still_works() {
    let r = Repo::new("demo");
    seed(&r);
    for role in ["spec-comprehension", "spec-critic", "spec-readiness"] {
        agent_toml(&r, role, "plan");
        r.agent_script(role, plan_pass_script());
    }
    r.wteam().args(["pickup"]).assert().success();
    r.wteam()
        .args(["phase", "plan-review"])
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
fn phase_validate_passes_default_chain() {
    let r = Repo::new("demo");
    seed(&r);
    for role in ["validate-local", "validate-mid", "validate-out"] {
        agent_toml(&r, role, "review");
        r.agent_script(role, validate_pass_script());
    }
    r.wteam().args(["pickup"]).assert().success();
    r.wteam()
        .args(["phase", "validate"])
        .assert()
        .success()
        .stdout(predicates::str::contains("PASSED"));
    assert!(
        r.path()
            .join(".wteam/demo/phases/validate/PASSED.md")
            .is_file()
    );
}

#[test]
fn phase_validate_stops_on_first_fail() {
    let r = Repo::new("demo");
    seed(&r);
    agent_toml(&r, "validate-local", "review");
    r.agent_script(
        "validate-local",
        r#"mkdir -p .wteam/demo/phases/validate
printf 'FINDINGS:\n- verify failed\n' > .wteam/demo/phases/validate/v.md
wteam phase-verdict fail --file .wteam/demo/phases/validate/v.md
"#,
    );
    for role in ["validate-mid", "validate-out"] {
        agent_toml(&r, role, "review");
        r.agent_script(role, validate_pass_script());
    }
    r.wteam().args(["pickup"]).assert().success();
    r.wteam()
        .args(["phase", "validate"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("verify failed"));
    assert!(
        !r.path()
            .join(".wteam/demo/phases/validate/PASSED.md")
            .is_file()
    );
    assert!(
        r.path()
            .join(".wteam/demo/phases/validate/01-validate-local.md")
            .is_file()
    );
    assert!(
        !r.path()
            .join(".wteam/demo/phases/validate/02-validate-mid.md")
            .is_file()
    );
}

#[test]
fn phase_persona_override() {
    let r = Repo::new("demo");
    seed(&r);
    agent_toml(&r, "validate-out", "review");
    r.agent_script("validate-out", validate_pass_script());
    r.wteam().args(["pickup"]).assert().success();
    r.wteam()
        .args(["phase", "validate", "--persona", "validate-out"])
        .assert()
        .success()
        .stdout(predicates::str::contains("validate-out"));
}

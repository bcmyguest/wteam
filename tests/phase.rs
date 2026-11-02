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

fn doc_pass_script(phase: &str) -> String {
    format!(
        r#"echo "$WTEAM_PROMPT" | grep -q 'Review the inputs for demo.' || exit 1
echo "$WTEAM_PROMPT" | grep -q 'docs/adr.md' || exit 1
mkdir -p .wteam/demo/phases/{phase}
printf 'FINDINGS:\n' > .wteam/demo/phases/{phase}/v.md
wteam phase-verdict pass --file .wteam/demo/phases/{phase}/v.md
"#
    )
}

fn doc_review_phase(r: &Repo) {
    r.write(
        ".wteam/phases/doc-review.toml",
        "personas = [\"doc-claims\", \"doc-scope\"]\nrequires_input = true\nbrief = \"Review the inputs for {slug}.\\n\"\n",
    );
    for role in ["doc-claims", "doc-scope"] {
        r.write(&format!(".wteam/roles/{role}.md"), &format!("# {role}\n"));
        agent_toml(r, role, "review");
        r.agent_script(role, &doc_pass_script("doc-review"));
    }
}

#[test]
fn phase_file_defines_a_new_phase_without_engine_code() {
    let r = Repo::new("demo");
    seed(&r);
    r.write("docs/adr.md", "# ADR\n\nUse SQLite.\n");
    doc_review_phase(&r);
    r.wteam().args(["pickup"]).assert().success();
    r.wteam()
        .args(["phase", "doc-review", "--input", "docs/adr.md"])
        .assert()
        .success()
        .stdout(predicates::str::contains("PASSED"));
    assert!(
        r.path()
            .join(".wteam/demo/phases/doc-review/02-doc-scope.md")
            .is_file()
    );
}

#[test]
fn phase_file_can_require_an_existing_input() {
    let r = Repo::new("demo");
    seed(&r);
    doc_review_phase(&r);
    r.wteam().args(["pickup"]).assert().success();
    r.wteam()
        .args(["phase", "doc-review"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("needs at least one --input"));
    r.wteam()
        .args(["phase", "doc-review", "--input", "docs/missing.md"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("is not a file"));
}

#[test]
fn project_phase_file_overrides_a_built_in() {
    let r = Repo::new("demo");
    seed(&r);
    r.write(
        ".wteam/phases/validate.toml",
        "personas = [\"validate-out\"]\n",
    );
    agent_toml(&r, "validate-out", "review");
    r.agent_script("validate-out", validate_pass_script());
    r.wteam().args(["pickup"]).assert().success();
    r.wteam()
        .args(["phase", "validate"])
        .assert()
        .success()
        .stdout(predicates::str::contains("validate-out"));
    assert!(
        r.path()
            .join(".wteam/demo/phases/validate/01-validate-out.md")
            .is_file()
    );
    assert!(
        !r.path()
            .join(".wteam/demo/phases/validate/02-validate-mid.md")
            .is_file()
    );
}

#[test]
fn unknown_phase_without_personas_names_the_phase_file() {
    let r = Repo::new("demo");
    seed(&r);
    r.wteam().args(["pickup"]).assert().success();
    r.wteam()
        .args(["phase", "nope"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("phases/nope.toml"));
}

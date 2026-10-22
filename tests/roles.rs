//! Guards role prompts and the skill against drift from the CLI surface.

use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn role(name: &str) -> String {
    std::fs::read_to_string(root().join("roles").join(format!("{name}.md"))).unwrap()
}

fn assert_mentions(name: &str, needles: &[&str]) {
    let text = role(name);
    for n in needles {
        assert!(text.contains(n), "roles/{name}.md must mention `{n}`");
    }
}

#[test]
fn planner_names_its_commands() {
    assert_mentions(
        "planner",
        &["wteam pickup", "Files:", "Verify:", "Blocked by:"],
    );
}

#[test]
fn orchestrator_names_its_commands() {
    assert_mentions(
        "orchestrator",
        &[
            "wteam ready",
            "wteam dispatch",
            "--pair",
            "wteam status",
            "wteam block",
            "--reason",
            "wteam send",
            "wteam requeue",
        ],
    );
}

#[test]
fn orchestrator_forbids_wait_and_defines_handoff() {
    let text = role("orchestrator");
    assert!(
        text.contains("wteam wait"),
        "must say why `wteam wait` is off limits"
    );
    for section in [
        "Goal",
        "Confirmed facts",
        "Ruled out",
        "Next steps",
        "Constraints",
        "Blockers",
    ] {
        assert!(text.contains(section), "handoff section {section} missing");
    }
}

#[test]
fn worker_names_its_commands() {
    assert_mentions(
        "worker",
        &[
            "wteam inbox",
            "wteam submit",
            "--summary",
            "wteam block",
            "--reason",
        ],
    );
}

#[test]
fn reviewer_names_its_commands() {
    assert_mentions(
        "reviewer",
        &[
            "wteam verdict",
            "verdict <ticket> pass|fail",
            "--finding",
            "--note",
            "--follow-up",
            "key:text",
        ],
    );
}

#[test]
fn skill_frontmatter_is_valid() {
    let md = std::fs::read_to_string(root().join("skills/wteam/SKILL.md")).unwrap();
    let front = md
        .strip_prefix("---\n")
        .and_then(|r| r.split_once("\n---\n"))
        .map(|(f, _)| f)
        .expect("frontmatter");
    assert!(front.lines().any(|l| l.trim() == "name: wteam"));
    let desc = front
        .lines()
        .find_map(|l| l.strip_prefix("description:"))
        .expect("description")
        .trim();
    assert!(
        !desc.is_empty() && desc.len() < 500,
        "description is {} chars",
        desc.len()
    );
}

#[test]
fn skill_names_each_step_command() {
    let md = std::fs::read_to_string(root().join("skills/wteam/SKILL.md")).unwrap();
    for n in [
        "wteam pickup --from",
        "wteam permissions",
        "wteam run --pairs",
        "--slug <slug>",
        "codex exec",
        "opencode run",
        "wteam report",
    ] {
        assert!(md.contains(n), "SKILL.md must mention `{n}`");
    }
}

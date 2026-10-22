//! Guards role prompts and the skill against drift from the CLI surface.

use std::path::PathBuf;

use serde_json::Value;
use support::Repo;

mod support;

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
            "--summary-file",
            "wteam block",
            "--reason-file",
        ],
    );
}

#[test]
fn reviewer_submits_verdict_via_file() {
    assert_mentions(
        "reviewer",
        &[
            "wteam verdict",
            "--file scratch/verdict-",
            "scratch/verdict-",
        ],
    );
}

#[test]
fn no_prompt_submits_free_text_inline() {
    for name in ["worker", "reviewer", "orchestrator"] {
        let text = role(name);
        assert!(
            !text.contains("--summary \""),
            "roles/{name}.md must not show `--summary \"`"
        );
    }
}

#[test]
fn worker_and_reviewer_retry_denied_command_via_file_form() {
    assert_mentions("worker", &["denied", "retry", "--summary-file"]);
    assert_mentions(
        "reviewer",
        &[
            "denied",
            "retry",
            "retry `wteam verdict NN pass|fail --file scratch/verdict-NN.md` once",
        ],
    );
    assert_eq!(
        role("reviewer").matches("itself is denied").count(),
        1,
        "roles/reviewer.md must have one denied rule, not two"
    );
}

#[test]
fn worker_and_reviewer_run_one_command_per_shell_call() {
    for name in ["worker", "reviewer"] {
        assert_mentions(
            name,
            &[
                "exactly one command per shell call",
                "no `&&`, `;` or `|`",
                "If `wteam` itself is denied",
                "naming the denied command",
            ],
        );
    }
}

#[test]
fn worker_and_reviewer_keep_scratch_in_repo() {
    for name in ["worker", "reviewer"] {
        assert_mentions(
            name,
            &[
                "`scratch/`",
                "Never write to `/tmp`",
                "Never submit `scratch/`",
            ],
        );
    }
}

#[test]
fn reviewer_names_its_commands() {
    assert_mentions(
        "reviewer",
        &[
            "wteam verdict",
            "verdict <ticket> pass|fail",
            "--file",
            "FINDINGS:",
            "NOTES:",
            "FOLLOW-UPS:",
            "key: text",
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

fn stdout_of(repo: &Repo, args: &[&str]) -> String {
    let out = repo.wteam().args(args).assert().success();
    String::from_utf8(out.get_output().stdout.clone()).unwrap()
}

fn role_json(repo: &Repo, name: &str) -> Value {
    serde_json::from_str(&stdout_of(repo, &["role", name, "--json"])).unwrap()
}

#[test]
fn role_built_in_prints_prompt() {
    let repo = Repo::new("alpha");
    let text = include_str!("../roles/planner.md");
    assert_eq!(
        stdout_of(&repo, &["role", "planner"]).trim_end(),
        text.trim_end()
    );
    let json = role_json(&repo, "planner");
    assert_eq!(json["name"], "planner");
    assert_eq!(json["source"], "built-in");
    assert_eq!(json["text"], text);
}

#[test]
fn role_project_override_wins() {
    let repo = Repo::new("alpha");
    repo.write(".wteam/roles/planner.md", "project planner\n");
    repo.write(".wteam-config/roles/planner.md", "config planner\n");
    let json = role_json(&repo, "planner");
    assert_eq!(json["source"], "project");
    assert_eq!(json["text"], "project planner\n");
}

#[test]
fn role_config_dir_beats_built_in() {
    let repo = Repo::new("alpha");
    repo.write(".wteam-config/roles/worker.md", "config worker\n");
    let json = role_json(&repo, "worker");
    assert_eq!(json["source"], "config");
    assert_eq!(json["text"], "config worker\n");
}

#[test]
fn role_missing_name_fails() {
    let repo = Repo::new("alpha");
    repo.wteam()
        .args(["role", "nope"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("no role prompt `nope`"));
}

#[test]
fn planner_role_runs_pickup_with_slug() {
    let text = include_str!("../roles/planner.md");
    assert!(
        text.contains("wteam pickup --slug <slug>"),
        "roles/planner.md must run `wteam pickup --slug <slug>`"
    );
}

#[test]
fn skill_step_two_uses_role_planner() {
    let md = std::fs::read_to_string(root().join("skills/wteam/SKILL.md")).unwrap();
    assert!(
        md.contains("wteam role planner"),
        "SKILL.md step 2 must run `wteam role planner`"
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

#[test]
fn worker_declares_extras_with_flag() {
    assert_mentions("worker", &["--extra", "outside `Files:`"]);
}

#[test]
fn reviewer_judges_extras() {
    assert_mentions("reviewer", &["Extra paths", "unexplained"]);
}

#[test]
fn worker_and_reviewer_run_from_repo_root() {
    for name in ["worker", "reviewer"] {
        assert_mentions(name, &["no `git -C`"]);
    }
}

#[test]
fn orchestrator_runs_one_command_per_shell_call_and_fills_pairs() {
    assert_mentions(
        "orchestrator",
        &[
            "exactly one command per shell call",
            "no `&&`, `;` or `|`",
            "If `wteam` itself is denied",
            "naming the denied command",
            "wteam dispatch --fill",
        ],
    );
}

#[test]
fn worker_and_reviewer_create_files_with_edit_tool() {
    for name in ["worker", "reviewer"] {
        assert_mentions(
            name,
            &[
                "1. Run `mkdir -p scratch` as its own command.",
                "2. Create `scratch/check_<name>.py` with the file-editing tool.",
                "3. Run `python3 scratch/check_<name>.py` as one line.",
                "Never write file contents through the shell",
                "Never run a multi-line command",
            ],
        );
    }
}

#[test]
fn worker_and_reviewer_advise_no_shell_file_creation() {
    for name in ["worker", "reviewer"] {
        let text = role(name);
        for advice in ["<<", "printf '", "cat >", "echo >"] {
            assert!(
                !text.contains(advice),
                "roles/{name}.md must not advise `{advice}`"
            );
        }
        assert!(
            !text
                .lines()
                .any(|l| l.contains("printf") && l.contains('>')),
            "roles/{name}.md must not show `printf` with `>`"
        );
    }
}

#[test]
fn worker_and_reviewer_keep_flag_values_on_one_line() {
    assert_mentions("worker", &["`--note` values on one line"]);
    assert_mentions("reviewer", &["each bullet in the verdict file on one line"]);
}

#[test]
fn replan_sections_name_their_commands() {
    assert_mentions(
        "planner",
        &["Re-plan", "wteam requeue", "wteam pickup --slug"],
    );
    assert_mentions("orchestrator", &["--replan"]);
}

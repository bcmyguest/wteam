//! `wteam pickup` through the binary: import, idempotence and each problem.

mod support;

use std::path::{Path, PathBuf};

use predicates::str::contains;
use support::Repo;

const SPEC: &str = "# Spec\n\n## Problem Statement\n\nUsers cannot import plans.\nSecond line of the paragraph.\n\nMore detail.\n\n## Solution\n\nAdd pickup.\n";

/// A `/to-tickets` output dir named `name` outside the repo (keeps the git tree clean).
fn source(name: &str) -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join(name);
    let issues = dir.join("issues");
    std::fs::create_dir_all(&issues).unwrap();
    std::fs::write(dir.join("spec.md"), SPEC).unwrap();
    std::fs::write(
        issues.join("01-first-thing.md"),
        ticket("01", "First thing", "None", ""),
    )
    .unwrap();
    std::fs::write(
        issues.join("02-second-thing.md"),
        ticket("02", "Second thing", "01", "true"),
    )
    .unwrap();
    (tmp, dir)
}

fn ticket(id: &str, title: &str, blocked_by: &str, verify: &str) -> String {
    let mut s = format!(
        "# {id}: {title}\r\n\r\n**What to build:** {title}.\n\n**Blocked by:** {blocked_by}\n\n**Status:** ready-for-agent\n\n"
    );
    if !verify.is_empty() {
        s.push_str(&format!("**Verify:** {verify}\n\n"));
    }
    s.push_str("- [ ] works\n");
    s
}

fn pickup(r: &Repo, from: &Path) -> assert_cmd::assert::Assert {
    r.wteam().arg("pickup").arg("--from").arg(from).assert()
}

fn json_pickup(r: &Repo, args: &[&str]) -> serde_json::Value {
    let out = r
        .wteam()
        .arg("--json")
        .arg("pickup")
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

#[test]
fn imports_tickets_and_spec_byte_for_byte() {
    let r = Repo::new("demo");
    let (_tmp, dir) = source("alpha");
    pickup(&r, &dir)
        .success()
        .stdout(contains("Picked up"))
        .stdout(contains("Goal"))
        .stdout(contains("Users cannot import plans."))
        .stdout(contains("Work graph"))
        .stdout(contains("flowchart TD"))
        .stdout(contains("Problems"))
        .stdout(contains("none"))
        .stdout(contains("Confirm to start: `wteam run --pairs N`"));
    for f in ["01-first-thing.md", "02-second-thing.md"] {
        let want = std::fs::read(dir.join("issues").join(f)).unwrap();
        let got = std::fs::read(r.effort().join("tickets").join(f)).unwrap();
        assert_eq!(want, got, "{f}");
    }
    assert_eq!(
        std::fs::read(dir.join("spec.md")).unwrap(),
        std::fs::read(r.effort().join("spec.md")).unwrap()
    );
}

#[test]
fn slug_defaults_to_source_dir_name() {
    let r = Repo::new("demo");
    let (_tmp, dir) = source("from-dir");
    r.wteam()
        .env_remove("WTEAM_SLUG")
        .arg("pickup")
        .arg("--from")
        .arg(&dir)
        .assert()
        .success();
    assert!(
        r.path()
            .join(".wteam/from-dir/tickets/01-first-thing.md")
            .is_file()
    );
    assert!(r.path().join(".wteam/from-dir/spec.md").is_file());
}

#[test]
fn rerun_is_idempotent_and_preserves_in_progress_tickets() {
    let r = Repo::new("demo");
    let (_tmp, dir) = source("alpha");
    pickup(&r, &dir).success();
    let first = json_pickup(&r, &["--from", dir.to_str().unwrap()]);
    let second = json_pickup(&r, &["--from", dir.to_str().unwrap()]);
    assert_eq!(first, second);

    // Ticket 01 is in progress: the worker's edit must survive a re-import.
    let edited = r.effort().join("tickets/01-first-thing.md");
    std::fs::write(&edited, "# 01: First thing\n\nedited by worker\n").unwrap();
    r.write(
        ".wteam/demo/state.json",
        &serde_json::json!({
            "slug": "demo",
            "tickets": { "01": { "status": "doing" } }
        })
        .to_string(),
    );
    // And a todo ticket with local edits is refreshed from the source.
    let todo = r.effort().join("tickets/02-second-thing.md");
    std::fs::write(&todo, "stale").unwrap();
    pickup(&r, &dir).success();
    assert_eq!(
        std::fs::read_to_string(&edited).unwrap(),
        "# 01: First thing\n\nedited by worker\n"
    );
    assert_eq!(
        std::fs::read(&todo).unwrap(),
        std::fs::read(dir.join("issues/02-second-thing.md")).unwrap()
    );
}

#[test]
fn json_returns_pickup_report() {
    let r = Repo::new("demo");
    let (_tmp, dir) = source("alpha");
    let v = json_pickup(&r, &["--from", dir.to_str().unwrap()]);
    assert_eq!(v["slug"], "demo");
    assert_eq!(
        v["goal"],
        "Users cannot import plans.\nSecond line of the paragraph."
    );
    assert_eq!(v["problems"], serde_json::json!([]));
    assert!(v["graph"].as_str().unwrap().starts_with("flowchart TD"));
    assert!(
        v["imported"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x.as_str().unwrap().contains("01-first-thing.md"))
    );
}

#[test]
fn goal_falls_back_to_solution_then_first_ticket_title() {
    let r = Repo::new("demo");
    let (_tmp, dir) = source("alpha");
    std::fs::write(
        dir.join("spec.md"),
        "# Spec\n\n## Solution\n\nAdd pickup.\n",
    )
    .unwrap();
    assert_eq!(
        json_pickup(&r, &["--from", dir.to_str().unwrap()])["goal"],
        "Add pickup."
    );
    std::fs::remove_file(dir.join("spec.md")).unwrap();
    std::fs::remove_file(r.effort().join("spec.md")).unwrap();
    assert_eq!(
        json_pickup(&r, &["--from", dir.to_str().unwrap()])["goal"],
        "First thing"
    );
}

#[test]
fn other_markdown_beside_issues_serves_as_spec() {
    let r = Repo::new("demo");
    let (_tmp, dir) = source("alpha");
    std::fs::remove_file(dir.join("spec.md")).unwrap();
    std::fs::write(dir.join("prd.md"), SPEC).unwrap();
    pickup(&r, &dir).success();
    assert_eq!(r.read(".wteam/demo/spec.md"), SPEC);
}

#[test]
fn verify_in_place_without_from() {
    let r = Repo::new("demo");
    r.ticket("01", "First thing", "None", "", "true");
    let v = json_pickup(&r, &[]);
    assert_eq!(v["slug"], "demo");
    assert_eq!(v["imported"], serde_json::json!([]));
    assert_eq!(v["problems"], serde_json::json!([]));
    assert_eq!(v["goal"], "First thing");
}

#[test]
fn dirty_tree_is_a_problem_with_exit_zero() {
    let r = Repo::new("demo");
    r.ticket("01", "First thing", "None", "", "");
    r.write("stray.txt", "dirty\n");
    let v = json_pickup(&r, &[]);
    let problems = v["problems"].as_array().unwrap();
    assert!(
        problems
            .iter()
            .any(|p| p.as_str().unwrap().contains("stray.txt")),
        "{problems:?}"
    );
    r.wteam()
        .arg("pickup")
        .assert()
        .success()
        .stdout(contains("stray.txt"));
}

#[test]
fn dirty_files_inside_wteam_are_ignored() {
    let r = Repo::new("demo");
    r.ticket("01", "First thing", "None", "", "");
    r.write(".wteam/notes.txt", "scratch\n");
    assert_eq!(json_pickup(&r, &[])["problems"], serde_json::json!([]));
}

#[test]
fn missing_verify_program_is_a_problem() {
    let r = Repo::new("demo");
    r.ticket(
        "01",
        "First thing",
        "None",
        "",
        "no-such-program-xyz --flag; cargo test",
    );
    let v = json_pickup(&r, &[]);
    let problems: Vec<&str> = v["problems"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_str().unwrap())
        .collect();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("no-such-program-xyz") && problems[0].contains("01"));
}

#[test]
fn unknown_blocker_is_a_problem() {
    let r = Repo::new("demo");
    r.ticket("01", "First thing", "99", "", "");
    let v = json_pickup(&r, &[]);
    assert!(
        v["problems"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p.as_str().unwrap().contains("unknown ticket 99"))
    );
    r.wteam()
        .arg("pickup")
        .assert()
        .success()
        .stdout(contains("unknown ticket 99"));
}

#[test]
fn source_inside_repo_scratch_dir_is_not_dirty() {
    let r = Repo::new("demo");
    let (_tmp, dir) = source("alpha");
    for f in [
        "spec.md",
        "issues/01-first-thing.md",
        "issues/02-second-thing.md",
    ] {
        r.write(
            &format!(".scratch/alpha/{f}"),
            &std::fs::read_to_string(dir.join(f)).unwrap(),
        );
    }
    let v = json_pickup(&r, &["--from", ".scratch/alpha"]);
    assert_eq!(v["problems"], serde_json::json!([]));
}

#[test]
fn relative_from_resolves_from_subdirectory_via_repo_root() {
    let r = Repo::new("demo");
    let (_tmp, dir) = source("alpha");
    for f in ["issues/01-first-thing.md", "issues/02-second-thing.md"] {
        r.write(
            &format!("plans/alpha/{f}"),
            &std::fs::read_to_string(dir.join(f)).unwrap(),
        );
    }
    std::fs::create_dir_all(r.path().join("sub")).unwrap();
    let out = r
        .wteam()
        .current_dir(r.path().join("sub"))
        .args(["--json", "pickup", "--from", "plans/alpha"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["problems"], serde_json::json!([]));
    assert!(r.effort().join("tickets/01-first-thing.md").is_file());
}

#[test]
fn untracked_source_outside_scratch_is_exempt_but_other_files_are_not() {
    let r = Repo::new("demo");
    let (_tmp, dir) = source("alpha");
    for f in ["issues/01-first-thing.md", "issues/02-second-thing.md"] {
        r.write(
            &format!("plans/alpha/{f}"),
            &std::fs::read_to_string(dir.join(f)).unwrap(),
        );
    }
    let v = json_pickup(&r, &["--from", "plans/alpha"]);
    assert_eq!(v["problems"], serde_json::json!([]));
    r.write("stray.txt", "x\n");
    let v = json_pickup(&r, &["--from", "plans/alpha"]);
    assert_eq!(v["problems"].as_array().unwrap().len(), 1);
}

#[test]
fn shell_builtins_and_chained_commands_are_checked_per_program() {
    let r = Repo::new("demo");
    r.ticket(
        "01",
        "First thing",
        "None",
        "",
        "cd . && no-such-prog-abc | cat; [ -f x ]",
    );
    let v = json_pickup(&r, &[]);
    let problems = v["problems"].as_array().unwrap();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].as_str().unwrap().contains("no-such-prog-abc"));
}

/// Tickets 01..=n in place, each blocked by its predecessor when `chain`, else only by 01.
/// Each carries a `**Context:**` section so only width warnings can appear.
fn graph(r: &Repo, n: usize, chain: bool) {
    for i in 1..=n {
        let blocked = match (i, chain) {
            (1, _) => "None (can start immediately)".to_string(),
            (_, true) => format!("{:02}", i - 1),
            (_, false) => "01".to_string(),
        };
        r.write(
            &format!(".wteam/demo/tickets/{i:02}-step-{i:02}.md"),
            &format!(
                "# {i:02}: Step {i:02}\n\n**What to build:** Step {i:02}.\n\n**Blocked by:** {blocked}\n\n**Context:**\n`src/a.rs:1`  `fn a()`  — stub\n\n- [ ] it works\n"
            ),
        );
    }
}

#[test]
fn ticket_without_context_warns_and_passes() {
    let r = Repo::new("demo");
    r.ticket("01", "First thing", "None", "", "");
    let v = json_pickup(&r, &[]);
    assert_eq!(v["problems"], serde_json::json!([]));
    let warnings = v["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(
        warnings[0].as_str().unwrap().contains("ticket 01"),
        "{warnings:?}"
    );
    r.wteam().arg("pickup").assert().success();
}

#[test]
fn ticket_with_context_has_no_context_warning() {
    let r = Repo::new("demo");
    r.write(
        ".wteam/demo/tickets/01-first-thing.md",
        "# 01: First thing\n\n**What to build:** First thing.\n\n**Blocked by:** None\n\n**Context:**\n`src/a.rs:1`  `fn a()`  — stub\n\n- [ ] it works\n",
    );
    let v = json_pickup(&r, &[]);
    assert_eq!(v["warnings"], serde_json::json!([]));
}

#[test]
fn eight_ticket_chain_warns_but_passes() {
    let r = Repo::new("demo");
    graph(&r, 8, true);
    let v = json_pickup(&r, &[]);
    assert_eq!(v["longest_chain"], 8);
    assert_eq!(v["max_parallel"], 1);
    assert_eq!(v["problems"], serde_json::json!([]));
    let warnings = v["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    let text = warnings[0].as_str().unwrap();
    assert!(text.contains("longest_chain 8"), "{text}");
    assert!(text.contains("max_parallel 1"), "{text}");
    r.wteam()
        .arg("pickup")
        .assert()
        .success()
        .stdout(contains("Warnings"))
        .stdout(contains("longest_chain 8"));
}

#[test]
fn ready_tickets_sharing_a_file_warn_to_allow_overlap() {
    let r = Repo::new("demo");
    for i in 1..=4 {
        r.write(
            &format!(".wteam/demo/tickets/{i:02}-step-{i:02}.md"),
            &format!(
                "# {i:02}: Step {i:02}\n\n**What to build:** Step {i:02}.\n\n**Blocked by:** None (can start immediately)\n\n**Files:** src/shared.rs\n\n**Context:**\n`src/shared.rs:1`  `fn a()`  — stub\n\n- [ ] it works\n"
            ),
        );
    }
    let v = json_pickup(&r, &[]);
    assert_eq!(v["problems"], serde_json::json!([]));
    assert_eq!(v["max_parallel"], 4);
    assert_eq!(v["max_parallel_serial"], 1);
    let warnings = v["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    let text = warnings[0].as_str().unwrap();
    assert!(text.contains("--overlap allow"), "{text}");
    assert!(text.contains("max_parallel_serial 1"), "{text}");
}

#[test]
fn tracer_with_six_fan_out_tickets_has_no_warning() {
    let r = Repo::new("demo");
    graph(&r, 7, false);
    let v = json_pickup(&r, &[]);
    assert_eq!(v["longest_chain"], 2);
    assert_eq!(v["max_parallel"], 6);
    assert_eq!(v["warnings"], serde_json::json!([]));
    assert_eq!(v["problems"], serde_json::json!([]));
}

#[test]
fn short_chains_below_four_tickets_do_not_warn() {
    let r = Repo::new("demo");
    graph(&r, 3, true);
    let v = json_pickup(&r, &[]);
    assert_eq!(v["longest_chain"], 3);
    assert_eq!(v["warnings"], serde_json::json!([]));
}

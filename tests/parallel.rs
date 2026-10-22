//! Parallel pairs and worktree isolation: disjoint tickets run concurrently, overlapping
//! tickets are refused, worktree merges land on main in order, and conflicts block.

mod support;

use predicates::str::contains;
use support::Repo;

/// Worker that waits for `go`, then edits its ticket's file and submits.
const WORKER: &str = r#"
id=$(printf '%s\n' "$WTEAM_PROMPT" | sed -n 's/.*[Tt]icket \([0-9][0-9]\).*/\1/p' | head -1)
i=0
while [ ! -f "$WTEAM_FAKE_DIR/go" ] && [ $i -lt 600 ]; do sleep 0.05; i=$((i+1)); done
mkdir -p src
echo edit >> "src/$id.txt"
wteam submit "$id" --summary "edited $id"
"#;

/// Reviewer that passes the review request it was sent.
const REVIEWER_PASS: &str = r#"
for id in $(printf '%s\n' "$WTEAM_PROMPT" | sed -n 's/^Review request: ticket \([0-9][0-9]\).*/\1/p'); do
  wteam verdict "$id" pass
done
"#;

/// Worker that parks on `go`, submits, and records each brief it was sent.
const RECORDING_WORKER: &str = r#"
id=$(printf '%s\n' "$WTEAM_PROMPT" | sed -n 's/.*[Tt]icket \([0-9][0-9]\).*/\1/p' | head -1)
printf '%s\n' "$WTEAM_PROMPT" >> "$WTEAM_FAKE_DIR/worker-prompts"
mkdir -p src
echo edit >> "src/$id.txt"
wteam submit "$id" --summary "edited $id"
"#;

/// Worker that leaves ticket 01 blocked with uncommitted work in its worktree.
const STUCK_WORKER: &str = r#"
id=$(printf '%s\n' "$WTEAM_PROMPT" | sed -n 's/.*[Tt]icket \([0-9][0-9]\).*/\1/p' | head -1)
mkdir -p src
if [ "$id" = "01" ]; then
  echo wip > src/wip.txt
  wteam block "$id" --reason "stuck"
  exit 0
fi
i=0
while [ ! -f "$WTEAM_FAKE_DIR/go" ] && [ $i -lt 600 ]; do sleep 0.05; i=$((i+1)); done
echo edit >> "src/$id.txt"
wteam submit "$id" --summary "edited $id"
"#;

/// Worker that stages an unrelated file and submits, so the ticket's commit has nothing to stage.
const STAGES_ELSEWHERE_WORKER: &str = r#"
id=$(printf '%s\n' "$WTEAM_PROMPT" | sed -n 's/.*[Tt]icket \([0-9][0-9]\).*/\1/p' | head -1)
echo other > other.txt
git add other.txt
wteam submit "$id" --summary "staged elsewhere"
"#;

/// Worktree isolation with `pairs` pairs and the default team; the orchestrator never dispatches.
fn repo_with_pairs(pairs: u32) -> Repo {
    let r = Repo::new("demo");
    r.use_fake_agents();
    r.agent_script("orchestrator", "echo ok\n");
    r.agent_script("planner", "echo ok\n");
    r.write(
        ".wteam/demo/state.json",
        &format!(
            r#"{{"slug":"demo","run":{{"pairs":{pairs},"isolation":"worktree",
                "transport":"resume","team":"default"}}}}"#
        ),
    );
    r.agent_script("worker", WORKER);
    r.agent_script("reviewer", REVIEWER_PASS);
    r
}

fn repo() -> Repo {
    repo_with_pairs(2)
}

/// Release the workers parked on the `go` file.
fn go(r: &Repo) {
    r.write(".wteam-fake/go", "");
}

#[test]
fn disjoint_tickets_run_concurrently_and_both_commit() {
    let r = repo();
    r.ticket("01", "Add one", "None", "src/01.txt", "");
    r.ticket("02", "Add two", "None", "src/02.txt", "");
    r.wteam().args(["dispatch", "01"]).assert().success();
    r.wteam().args(["dispatch", "02"]).assert().success();

    // Both workers are parked on `go`, so both being in progress proves they run at once.
    let s = r.wait_for(60, |s| {
        s["tickets"]["01"]["status"] == "doing" && s["tickets"]["02"]["status"] == "doing"
    });
    assert_eq!(s["tickets"]["01"]["pair"], 1);
    assert_eq!(s["tickets"]["02"]["pair"], 2);
    assert!(r.path().join(".wteam/demo/worktrees/pair1").is_dir());
    assert!(r.path().join(".wteam/demo/worktrees/pair2").is_dir());
    go(&r);

    r.wait_for(60, |s| {
        s["tickets"]["01"]["status"] == "done" && s["tickets"]["02"]["status"] == "done"
    });
    let log = r.git(&["log", "--format=%s"]);
    assert!(log.contains("feat: Add one (wteam 01)"), "{log}");
    assert!(log.contains("feat: Add two (wteam 02)"), "{log}");
    assert!(r.read("src/01.txt").contains("edit"));
    assert!(r.read("src/02.txt").contains("edit"));
}

#[test]
fn overlapping_ticket_is_refused_until_the_first_passes() {
    let r = repo();
    r.ticket("01", "Add one", "None", "src/01.txt", "");
    r.ticket("02", "Add two", "None", "src/02.txt", "");
    r.ticket("03", "Add three", "None", "src/01.txt", "");
    r.wteam().args(["dispatch", "01"]).assert().success();

    r.wteam()
        .args(["dispatch", "03"])
        .assert()
        .failure()
        .stderr(contains("overlaps files with 01"));

    let out = r.wteam().args(["ready", "--json"]).output().unwrap();
    let ready: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let flag = |id: &str| {
        ready
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["id"] == id)
            .map(|t| t["dispatchable"].clone())
    };
    assert_eq!(flag("02"), Some(true.into()));
    assert_eq!(flag("03"), Some(false.into()));

    go(&r);
    r.wait_for(60, |s| s["tickets"]["01"]["status"] == "done");
    r.wteam().args(["dispatch", "03"]).assert().success();
    r.wait_for(60, |s| s["tickets"]["03"]["status"] == "done");
}

#[test]
fn worktree_merges_land_in_order_and_worktrees_are_removed() {
    let r = repo();
    r.ticket("01", "Add one", "None", "src/01.txt", "");
    r.ticket("02", "Add two", "01", "src/02.txt", "");
    r.wteam().args(["dispatch", "01"]).assert().success();
    r.wait_for(60, |s| s["tickets"]["01"]["status"] == "doing");
    assert!(r.path().join(".wteam/demo/worktrees/pair1").is_dir());
    go(&r);
    r.wait_for(60, |s| s["tickets"]["01"]["status"] == "done");
    r.wteam().args(["dispatch", "02"]).assert().success();
    r.wait_for(60, |s| s["tickets"]["02"]["status"] == "done");

    let log = r.git(&["log", "--format=%s"]);
    let lines: Vec<&str> = log.lines().collect();
    assert!(lines[0].contains("(wteam 02)"), "{log}");
    assert!(lines[1].contains("(wteam 01)"), "{log}");
    assert!(r.git(&["log", "--merges", "--format=%H"]).is_empty());
    assert!(!r.path().join(".wteam/demo/worktrees/pair1").exists());
    assert!(!r.path().join(".wteam/demo/worktrees/pair2").exists());
    assert_eq!(r.git(&["worktree", "list"]).lines().count(), 1);
    assert!(r.git(&["branch", "--list", "wteam/*"]).trim().is_empty());
}

#[test]
fn conflicting_worktree_merge_blocks_ticket_and_leaves_main_alone() {
    let r = repo();
    r.ticket("01", "Add one", "None", "src/01.txt", "");
    r.wteam().args(["dispatch", "01"]).assert().success();
    r.wait_for(60, |s| s["tickets"]["01"]["status"] == "doing");

    // Main gains the same file while the worker's branch is still on the old base.
    r.write("src/01.txt", "main\n");
    r.git(&["add", "src/01.txt"]);
    r.git(&["commit", "-q", "-m", "chore: main edit"]);
    let main_head = r.git(&["rev-parse", "HEAD"]);
    go(&r);

    let s = r.wait_for(60, |s| s["tickets"]["01"]["status"] == "blocked");
    let reason = s["tickets"]["01"]["blocked_reason"].as_str().unwrap();
    assert!(reason.contains("src/01.txt"), "{reason}");
    assert_eq!(r.git(&["rev-parse", "HEAD"]), main_head);
    assert!(!r.git(&["log", "--format=%s"]).contains("(wteam 01)"));
}

#[test]
fn dirty_pair_is_skipped_by_auto_pick_and_reported() {
    let r = repo();
    r.agent_script("worker", STUCK_WORKER);
    r.agent_script("reviewer", REVIEWER_PASS);
    r.ticket("01", "Add one", "None", "src/01.txt", "");
    r.ticket("02", "Add two", "None", "src/02.txt", "");
    r.wteam().args(["dispatch", "01"]).assert().success();
    r.wait_for(60, |s| s["tickets"]["01"]["status"] == "blocked");
    r.write(".wteam/demo/worktrees/pair1/src/stray.txt", "left behind");
    go(&r);

    r.wteam().args(["dispatch", "02"]).assert().success();
    let s = r.wait_for(60, |s| s["tickets"]["02"]["status"] == "done");
    assert_eq!(s["tickets"]["02"]["pair"], 2);

    r.wteam()
        .arg("status")
        .assert()
        .success()
        .stdout(contains("dirty worktrees (uncommitted work): pair 1"));
    let out = r.wteam().args(["status", "--json"]).output().unwrap();
    let status: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(status["dirty_worktrees"], serde_json::json!([1]));
    r.wteam()
        .arg("stop")
        .assert()
        .success()
        .stdout(contains("Dirty worktrees: pair 1"));
}

#[test]
fn auto_dispatch_refuses_when_every_idle_pair_is_dirty() {
    let r = repo_with_pairs(1);
    r.agent_script("worker", STUCK_WORKER);
    r.agent_script("reviewer", REVIEWER_PASS);
    r.ticket("01", "Add one", "None", "src/01.txt", "");
    r.ticket("02", "Add two", "None", "src/02.txt", "");
    r.wteam().args(["dispatch", "01"]).assert().success();
    r.wait_for(60, |s| s["tickets"]["01"]["status"] == "blocked");
    r.write(".wteam/demo/worktrees/pair1/src/stray.txt", "left behind");

    r.wteam()
        .args(["dispatch", "02"])
        .assert()
        .failure()
        .stderr(contains("dirty: pair 1"));
}

#[test]
fn blocked_work_is_committed_to_its_branch_and_pair_is_reused() {
    let r = repo_with_pairs(1);
    r.agent_script("worker", STUCK_WORKER);
    r.agent_script("reviewer", REVIEWER_PASS);
    r.ticket("01", "Add one", "None", "src/01.txt", "");
    r.ticket("02", "Add two", "None", "src/02.txt", "");
    r.wteam().args(["dispatch", "01"]).assert().success();
    r.wait_for(60, |s| s["tickets"]["01"]["status"] == "blocked");

    let pair = ".wteam/demo/worktrees/pair1";
    assert_eq!(r.git(&["-C", pair, "status", "--porcelain"]), "");
    assert_eq!(
        r.git(&["log", "-1", "--format=%s", "wteam/demo/01"]).trim(),
        "wip: Add one (wteam 01, blocked)"
    );
    assert!(
        r.git(&["ls-tree", "-r", "--name-only", "wteam/demo/01"])
            .contains("src/wip.txt")
    );

    go(&r);
    r.wteam().args(["dispatch", "02"]).assert().success();
    let s = r.wait_for(60, |s| s["tickets"]["02"]["status"] == "done");
    assert_eq!(s["tickets"]["02"]["pair"], 1);
}

#[test]
fn requeued_ticket_resumes_on_its_preserved_branch() {
    let r = repo_with_pairs(1);
    r.agent_script("worker", STUCK_WORKER);
    r.agent_script("reviewer", REVIEWER_PASS);
    r.ticket("01", "Add one", "None", "src/01.txt", "");
    r.wteam().args(["dispatch", "01"]).assert().success();
    r.wait_for(60, |s| s["tickets"]["01"]["status"] == "blocked");

    r.agent_script("worker", WORKER);
    r.wteam().args(["requeue", "01"]).assert().success();
    r.wteam().args(["dispatch", "01"]).assert().success();
    assert!(
        r.path()
            .join(".wteam/demo/worktrees/pair1/src/wip.txt")
            .exists()
    );
    go(&r);
    r.wait_for(60, |s| s["tickets"]["01"]["status"] == "done");
}

#[test]
fn commit_failure_blocks_the_ticket_instead_of_stranding_it_in_review() {
    let r = repo();
    r.agent_script("worker", STAGES_ELSEWHERE_WORKER);
    r.agent_script("reviewer", REVIEWER_PASS);
    r.ticket("01", "Add one", "None", "src/01.txt", "");
    let main_head = r.git(&["rev-parse", "HEAD"]);
    r.wteam().args(["dispatch", "01"]).assert().success();

    let s = r.wait_for(60, |s| s["tickets"]["01"]["status"] == "blocked");
    let reason = s["tickets"]["01"]["blocked_reason"].as_str().unwrap();
    assert!(
        reason.contains("nothing in the ticket's files changed"),
        "{reason}"
    );
    assert_eq!(r.git(&["rev-parse", "HEAD"]), main_head);
}

#[test]
fn unrelated_dirty_file_in_main_survives_merge_back() {
    let r = repo();
    r.agent_script("worker", WORKER);
    r.agent_script("reviewer", REVIEWER_PASS);
    r.ticket("01", "Add one", "None", "src/01.txt", "");
    r.wteam().args(["dispatch", "01"]).assert().success();
    r.wait_for(60, |s| s["tickets"]["01"]["status"] == "doing");

    r.write("README.md", "scratch\n");
    go(&r);
    r.wait_for(60, |s| s["tickets"]["01"]["status"] == "done");

    assert_eq!(r.read("README.md"), "scratch\n");
    assert!(r.git(&["status", "--porcelain"]).contains("README.md"));
    assert!(r.read("src/01.txt").contains("edit"));
}

#[test]
fn worktree_brief_points_at_the_contract_by_absolute_path() {
    let r = repo();
    r.write(".wteam/demo/architecture.md", "# contract\n");
    r.agent_script("worker", RECORDING_WORKER);
    r.agent_script("reviewer", REVIEWER_PASS);
    r.ticket("01", "Add one", "None", "src/01.txt", "");
    r.wteam().args(["dispatch", "01"]).assert().success();
    r.wait_for(60, |s| s["tickets"]["01"]["status"] == "done");

    let contract = r.path().join(".wteam/demo/architecture.md");
    let brief = r.read(".wteam-fake/worker-prompts");
    assert!(
        brief.contains(&contract.display().to_string()),
        "brief does not name {}: {brief}",
        contract.display()
    );
    assert!(contract.is_file());
}

#[test]
fn fill_dispatches_lowest_ready_tickets_to_each_idle_pair() {
    let r = repo_with_pairs(4);
    for n in 1..=6 {
        r.ticket(
            &format!("0{n}"),
            &format!("Add {n}"),
            "None",
            &format!("src/0{n}.txt"),
            "",
        );
    }
    let out = r.wteam().args(["dispatch", "--fill"]).assert().success();
    let text = String::from_utf8_lossy(&out.get_output().stdout).to_string();
    for n in 1..=4 {
        assert!(
            text.contains(&format!("dispatched 0{n} to worker.{n}")),
            "{text}"
        );
    }
    assert!(!text.contains("dispatched 05"), "{text}");

    let s = r.state();
    for n in 1..=4 {
        assert_eq!(s["tickets"][format!("0{n}")]["status"], "doing", "{s:#}");
        assert_eq!(s["tickets"][format!("0{n}")]["pair"], n, "{s:#}");
    }
    assert_ne!(s["tickets"]["05"]["status"], "doing", "{s:#}");
    go(&r);
}

#[test]
fn fill_skips_a_ticket_whose_files_overlap_one_in_progress() {
    let r = repo_with_pairs(4);
    r.ticket("01", "Add one", "None", "src/a.txt", "");
    r.ticket("02", "Add two", "None", "src/a.txt", "");
    r.ticket("03", "Add three", "None", "src/b.txt", "");
    let out = r.wteam().args(["dispatch", "--fill"]).assert().success();
    let text = String::from_utf8_lossy(&out.get_output().stdout).to_string();
    assert!(text.contains("dispatched 01 to"), "{text}");
    assert!(text.contains("dispatched 03 to"), "{text}");
    assert!(!text.contains("dispatched 02"), "{text}");
    assert_ne!(r.state()["tickets"]["02"]["status"], "doing");
    go(&r);
}

#[test]
fn fill_with_no_idle_pair_says_nothing_to_dispatch() {
    let r = repo_with_pairs(1);
    r.ticket("01", "Add one", "None", "src/01.txt", "");
    r.ticket("02", "Add two", "None", "src/02.txt", "");
    r.wteam().args(["dispatch", "01"]).assert().success();

    r.wteam()
        .args(["dispatch", "--fill"])
        .assert()
        .success()
        .stdout(contains("nothing to dispatch"));
    let out = r
        .wteam()
        .args(["dispatch", "--fill", "--json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        json["dispatched"].as_array().map(Vec::len),
        Some(0),
        "{json:#}"
    );
    go(&r);
}

#[test]
fn fill_with_a_ticket_id_is_an_error() {
    let r = repo_with_pairs(2);
    r.ticket("01", "Add one", "None", "src/01.txt", "");
    r.wteam()
        .args(["dispatch", "01", "--fill"])
        .assert()
        .failure();
    assert_eq!(
        r.state()["tickets"]["01"]["status"],
        serde_json::Value::Null
    );
}

#[test]
fn fill_error_midway_still_lists_the_dispatches_that_stand() {
    let r = repo_with_pairs(2);
    r.ticket("01", "Add one", "None", "src/01.txt", "");
    r.ticket("02", "Add two", "None", "src/02.txt", "");
    // A plain file where pair 2's worktree goes makes its dispatch fail after pair 1 started.
    r.write(".wteam/demo/worktrees/pair2", "not a worktree\n");
    let out = r
        .wteam()
        .args(["dispatch", "--fill", "--json"])
        .assert()
        .failure();
    let json: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert_eq!(json["dispatched"][0]["ticket"], "01", "{json:#}");
    assert!(json["error"].as_str().is_some(), "{json:#}");
    assert_eq!(r.state()["tickets"]["01"]["status"], "doing");
    go(&r);
}

/// Worker that blocks its first attempt with a wip commit and a scratch file, then edits only
/// its ticket's file.
const WIP_THEN_CLEAN_WORKER: &str = r#"
id=$(printf '%s\n' "$WTEAM_PROMPT" | sed -n 's/.*[Tt]icket \([0-9][0-9]\).*/\1/p' | head -1)
mkdir -p src scratch
if [ ! -f "$WTEAM_FAKE_DIR/tried" ]; then
  touch "$WTEAM_FAKE_DIR/tried"
  echo wip > src/wip.txt
  echo notes > scratch/notes.txt
  wteam block "$id" --reason "stuck"
  exit 0
fi
echo edit >> "src/$id.txt"
wteam submit "$id" --summary "edited $id"
"#;

#[test]
fn landed_ticket_squashes_its_wip_history_into_one_commit_of_its_files() {
    let r = repo_with_pairs(1);
    r.agent_script("worker", WIP_THEN_CLEAN_WORKER);
    r.agent_script("reviewer", REVIEWER_PASS);
    r.ticket("01", "Add one", "None", "src/01.txt", "");
    let before: u32 = r
        .git(&["rev-list", "--count", "main"])
        .trim()
        .parse()
        .unwrap();
    r.wteam().args(["dispatch", "01"]).assert().success();
    r.wait_for(60, |s| s["tickets"]["01"]["status"] == "blocked");
    r.wteam().args(["requeue", "01"]).assert().success();
    r.wteam().args(["dispatch", "01"]).assert().success();
    r.wait_for(60, |s| s["tickets"]["01"]["status"] == "done");

    let after: u32 = r
        .git(&["rev-list", "--count", "main"])
        .trim()
        .parse()
        .unwrap();
    assert_eq!(after, before + 1);
    assert_eq!(
        r.git(&["log", "-1", "--format=%s", "main"]).trim(),
        "feat: Add one (wteam 01)"
    );
    assert_eq!(
        r.git(&["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"])
            .trim(),
        "src/01.txt"
    );
    assert!(!r.path().join("scratch/notes.txt").exists());
}

/// A ticket without `Files:` commits everything it touched except `scratch/`.
#[test]
fn filesless_ticket_commits_everything_but_scratch() {
    let r = repo_with_pairs(1);
    r.agent_script(
        "worker",
        r#"
id=$(printf '%s\n' "$WTEAM_PROMPT" | sed -n 's/.*[Tt]icket \([0-9][0-9]\).*/\1/p' | head -1)
mkdir -p src scratch
echo notes > scratch/notes.txt
echo edit >> "src/$id.txt"
wteam submit "$id" --summary "edited $id"
"#,
    );
    r.agent_script("reviewer", REVIEWER_PASS);
    r.ticket("01", "Add one", "None", "", "");
    r.wteam().args(["dispatch", "01"]).assert().success();
    r.wait_for(60, |s| s["tickets"]["01"]["status"] == "done");

    assert_eq!(
        r.git(&["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"])
            .trim(),
        "src/01.txt"
    );
}

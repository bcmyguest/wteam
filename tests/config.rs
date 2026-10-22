//! `wteam config` (promote, list, show), `wteam setup`, and the run transport default.

mod support;

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use predicates::str::contains;
use support::Repo;

/// Config dir the harness points `WTEAM_CONFIG_DIR` at.
fn config_dir(repo: &Repo) -> std::path::PathBuf {
    repo.path().join(".wteam-config")
}

/// A `herdr` script on a fresh PATH dir that prints `version` for `--version`.
/// Returns the PATH value to pass to `wteam`: the fake dir, then the wteam binary dir.
fn fake_herdr(repo: &Repo, version: Option<&str>) -> String {
    let bin = repo.path().join(".fake-bin");
    std::fs::create_dir_all(&bin).unwrap();
    if let Some(v) = version {
        let script = bin.join("herdr");
        std::fs::write(&script, format!("#!/bin/sh\necho \"herdr {v}\"\n")).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let wteam_dir = support::path_with_wteam();
    let wteam_dir = wteam_dir.split(':').next().unwrap().to_string();
    format!("{}:{wteam_dir}", bin.display())
}

#[test]
fn promote_list_show_round_trip() {
    let repo = Repo::new("slug");
    repo.wteam()
        .args(["config", "promote", "mine"])
        .assert()
        .success()
        .stdout(contains("team.dot"))
        .stdout(contains("agents/worker.toml"))
        .stdout(contains("profiles/edit.toml"));

    let saved = config_dir(&repo).join("teams/mine");
    assert!(saved.join("team.dot").is_file());
    assert!(saved.join("agents/worker.toml").is_file());
    assert!(saved.join("profiles/edit.toml").is_file());

    let list: serde_json::Value = serde_json::from_slice(
        &repo
            .wteam()
            .args(["config", "list", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )
    .unwrap();
    let text = list.to_string();
    assert!(text.contains("mine"), "list json: {text}");

    repo.wteam()
        .args(["config", "list"])
        .assert()
        .success()
        .stdout(contains("mine"));
    repo.wteam()
        .args(["config", "show", "mine"])
        .assert()
        .success()
        .stdout(contains("digraph"))
        .stdout(contains("worker"));
    repo.wteam()
        .args(["config", "show", "missing"])
        .assert()
        .failure()
        .stderr(contains("missing"));
}

#[test]
fn promote_twice_reports_changes_and_overwrites() {
    let repo = Repo::new("slug");
    repo.wteam()
        .args(["config", "promote", "mine"])
        .assert()
        .success();

    // Change the effort's worker agent, then promote again.
    repo.write(
        ".wteam/agents/worker.toml",
        "name = \"worker\"\ncli = \"fake\"\nprompt = \"worker\"\nprofile = \"edit\"\n",
    );
    repo.wteam()
        .args(["config", "promote", "mine"])
        .assert()
        .success()
        .stdout(contains("agents/worker.toml updated"))
        .stdout(contains("team.dot unchanged"));

    let saved =
        std::fs::read_to_string(config_dir(&repo).join("teams/mine/agents/worker.toml")).unwrap();
    assert!(saved.contains("cli = \"fake\""), "saved: {saved}");
}

#[test]
fn promote_rejects_bad_names() {
    let repo = Repo::new("slug");
    repo.wteam()
        .args(["config", "promote", "../escape"])
        .assert()
        .failure();
    assert!(!Path::new(&config_dir(&repo).join("escape")).exists());
}

#[test]
fn promoted_team_agents_resolve_for_run() {
    let repo = Repo::new("slug");
    // Every role runs the fake CLI. The orchestrator's script leaves a marker. Built-in
    // definitions use the `claude` CLI, which the test machine lacks, so the marker can only
    // appear if the promoted orchestrator definition was resolved.
    repo.use_fake_agents();
    repo.agent_script(
        "orchestrator",
        "echo ran >> \"$WTEAM_FAKE_DIR/promoted-orch\"\necho done\n",
    );
    repo.wteam()
        .args(["config", "promote", "mine"])
        .assert()
        .success();
    std::fs::remove_dir_all(repo.path().join(".wteam/agents")).unwrap();
    repo.wteam()
        .args(["run", "--team", "mine", "--timeout", "20"])
        .assert()
        .success();
    assert!(
        repo.fake_dir().join("promoted-orch").is_file(),
        "orchestrator marker missing: promoted definition was not used"
    );
}

#[test]
fn promote_prunes_files_no_longer_used() {
    let repo = Repo::new("slug");
    repo.write("docs/my-role.md", "path role\n");
    repo.write(
        ".wteam/agents/worker.toml",
        "name = \"worker\"\ncli = \"claude\"\nprompt = \"docs/my-role.md\"\nprofile = \"edit\"\n",
    );
    repo.wteam()
        .args(["config", "promote", "mine"])
        .assert()
        .success();
    let role = config_dir(&repo).join("teams/mine/roles/my-role.md");
    assert!(role.is_file());

    // The worker no longer uses the role file: the next promote removes it.
    repo.write(
        ".wteam/agents/worker.toml",
        "name = \"worker\"\ncli = \"claude\"\nprofile = \"edit\"\n",
    );
    repo.wteam()
        .args(["config", "promote", "mine"])
        .assert()
        .success()
        .stdout(contains("roles/my-role.md removed"));
    assert!(!role.exists());
}

#[test]
fn setup_writes_herdr_when_detected() {
    let repo = Repo::new("slug");
    let path = fake_herdr(&repo, Some("0.9.5"));
    repo.wteam()
        .env("PATH", &path)
        .args(["setup", "--yes"])
        .assert()
        .success();
    let cfg = std::fs::read_to_string(config_dir(&repo).join("config.toml")).unwrap();
    assert!(cfg.contains("transport = \"herdr\""), "config: {cfg}");
}

#[test]
fn setup_falls_back_to_resume_and_prints_install_command() {
    let repo = Repo::new("slug");
    // Old herdr: below the 0.9.5 floor.
    let path = fake_herdr(&repo, Some("0.9.4"));
    repo.wteam()
        .env("PATH", &path)
        .args(["setup", "--yes"])
        .assert()
        .success()
        .stdout(contains("curl -fsSL https://herdr.dev/install.sh | sh"));
    let cfg = std::fs::read_to_string(config_dir(&repo).join("config.toml")).unwrap();
    assert!(cfg.contains("transport = \"resume\""), "config: {cfg}");
}

#[test]
fn setup_without_herdr_prints_install_command() {
    let repo = Repo::new("slug");
    // No herdr anywhere on PATH.
    let path = fake_herdr(&repo, None);
    repo.wteam()
        .env("PATH", &path)
        .args(["setup"])
        .assert()
        .success()
        .stdout(contains("curl -fsSL https://herdr.dev/install.sh | sh"));
    let cfg = std::fs::read_to_string(config_dir(&repo).join("config.toml")).unwrap();
    assert!(cfg.contains("transport = \"resume\""), "config: {cfg}");
}

#[test]
fn run_reads_transport_default_from_config() {
    let repo = Repo::new("slug");
    repo.write(".wteam-config/config.toml", "transport = \"bogus\"\n");
    repo.wteam()
        .args(["run", "--timeout", "1"])
        .assert()
        .failure()
        .stderr(contains("unknown transport `bogus`"));
}

#[test]
fn promote_copies_role_prompt_files() {
    let repo = Repo::new("slug");
    // Path prompt: copied to roles/<stem>.md and the saved agent names the stem.
    repo.write("docs/my-role.md", "path role\n");
    repo.write(
        ".wteam/agents/worker.toml",
        "name = \"worker\"\ncli = \"claude\"\nprompt = \"docs/my-role.md\"\nprofile = \"edit\"\n",
    );
    // Bare name: found as .wteam/roles/<name>.md and copied as-is.
    repo.write(".wteam/roles/custom.md", "bare role\n");
    repo.write(
        ".wteam/agents/reviewer.toml",
        "name = \"reviewer\"\ncli = \"claude\"\nprompt = \"custom\"\nprofile = \"review\"\n",
    );
    repo.wteam()
        .args(["config", "promote", "mine"])
        .assert()
        .success()
        .stdout(contains("roles/my-role.md new"))
        .stdout(contains("roles/custom.md new"));

    let saved = config_dir(&repo).join("teams/mine");
    assert_eq!(
        std::fs::read_to_string(saved.join("roles/my-role.md")).unwrap(),
        "path role\n"
    );
    assert_eq!(
        std::fs::read_to_string(saved.join("roles/custom.md")).unwrap(),
        "bare role\n"
    );
    let worker = std::fs::read_to_string(saved.join("agents/worker.toml")).unwrap();
    assert!(worker.contains("prompt = \"my-role\""), "worker: {worker}");
    let reviewer = std::fs::read_to_string(saved.join("agents/reviewer.toml")).unwrap();
    assert!(
        reviewer.contains("prompt = \"custom\""),
        "reviewer: {reviewer}"
    );
}

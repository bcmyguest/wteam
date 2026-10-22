//! `wteam permissions` black-box tests.

mod support;

use predicates::str::contains;
use support::Repo;

#[test]
fn no_args_shows_every_builtin_across_four_clis() {
    let r = Repo::new("demo");
    let out = r.wteam().arg("permissions").assert().success();
    let text = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    for p in ["control", "review", "edit", "full"] {
        assert!(text.lines().any(|l| l == p), "missing {p}: {text}");
    }
    for c in ["claude", "codex", "opencode", "cursor"] {
        assert!(text.contains(c), "missing {c}: {text}");
    }
    assert!(text.contains('✔') && text.contains('≈'), "{text}");
}

#[test]
fn filters_by_profile_and_cli() {
    let r = Repo::new("demo");
    let out = r
        .wteam()
        .args(["permissions", "--profile", "edit", "--cli", "cursor"])
        .assert()
        .success();
    let text = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(text.contains("cursor") && text.contains("≈ deny"), "{text}");
    assert!(
        !text.contains("codex") && !text.lines().any(|l| l == "review"),
        "{text}"
    );
}

#[test]
fn unknown_profile_lists_builtins() {
    let r = Repo::new("demo");
    r.wteam()
        .args(["permissions", "--profile", "nope"])
        .assert()
        .failure()
        .stderr(contains("control, review, edit, full"));
}

#[test]
fn custom_profile_from_project_and_config_dir() {
    let r = Repo::new("demo");
    r.write(
        ".wteam/profiles/mine.toml",
        "name = \"mine\"\nallow = [\"Bash(make *)\"]\n",
    );
    r.write(
        ".wteam-config/profiles/global.toml",
        "name = \"global\"\nask = [\"Bash(git push *)\"]\n",
    );
    r.wteam()
        .args(["permissions", "--profile", "mine", "--cli", "codex"])
        .assert()
        .success()
        .stdout(contains("Bash(make *)"));
    r.wteam()
        .args(["permissions", "--profile", "global", "--cli", "claude"])
        .assert()
        .success()
        .stdout(contains("Bash(git push *)"));
}

#[test]
fn project_profile_overrides_builtin() {
    let r = Repo::new("demo");
    r.write(
        ".wteam/profiles/edit.toml",
        "name = \"edit\"\nallow = [\"Bash(only *)\"]\n",
    );
    let out = r
        .wteam()
        .args(["permissions", "--profile", "edit", "--cli", "claude"])
        .assert()
        .success();
    let text = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(
        text.contains("Bash(only *)") && !text.contains("Read(**)"),
        "{text}"
    );
}

#[test]
fn json_returns_support_data() {
    let r = Repo::new("demo");
    let out = r
        .wteam()
        .args([
            "permissions",
            "--json",
            "--profile",
            "edit",
            "--cli",
            "cursor",
            "--cli",
            "claude",
        ])
        .assert()
        .success();
    let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert_eq!(v["clis"], serde_json::json!(["cursor", "claude"]));
    let rules = v["profiles"][0]["rules"].as_array().unwrap();
    let push = rules
        .iter()
        .find(|r| r["rule"] == "Bash(git push *)")
        .unwrap();
    assert_eq!(push["decision"], "ask");
    assert_eq!(push["support"]["cursor"]["support"], "lossy");
    assert_eq!(push["support"]["claude"]["support"], "exact");
}

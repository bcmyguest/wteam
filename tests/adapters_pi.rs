//! Golden tests for the pi adapter over constructed fixtures, and `wteam permissions --cli pi`.

mod support;

use std::path::{Path, PathBuf};

use predicates::str::contains;
use support::Repo;
use wteam::adapters::{Adapter, TurnRequest, pi::Pi};
use wteam::permissions::Translation;

const SESSION: &str = "0f6c1a2e-7b3d-4c8e-9a1f-2d5b6e7c8a90";

fn fixture(rel: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/adapters/pi")
        .join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

fn base() -> TurnRequest {
    TurnRequest {
        agent_id: "worker.1".into(),
        model: None,
        session_id: None,
        prompt: "list files".into(),
        role_prompt: None,
        cwd: PathBuf::from("/work"),
        env: Vec::new(),
        extra_args: Vec::new(),
        permissions: Translation::default(),
    }
}

#[test]
fn first_turn_command() {
    let spec = Pi.command(&base());
    assert_eq!(spec.program, "pi");
    assert_eq!(spec.args, s(&["--mode", "json", "--", "list files"]));
    assert_eq!(spec.cwd, PathBuf::from("/work"));
    assert_eq!(spec.stdin, None);
}

#[test]
fn model_flag_precedes_prompt() {
    let req = TurnRequest {
        model: Some("openai/gpt-5:high".into()),
        ..base()
    };
    assert_eq!(
        Pi.command(&req).args,
        s(&[
            "--mode",
            "json",
            "--model",
            "openai/gpt-5:high",
            "--",
            "list files"
        ])
    );
}

#[test]
fn role_prompt_is_appended_as_system_prompt_on_first_turn() {
    let req = TurnRequest {
        role_prompt: Some("ROLE TEXT".into()),
        ..base()
    };
    assert_eq!(
        Pi.command(&req).args,
        s(&[
            "--mode",
            "json",
            "--append-system-prompt",
            "ROLE TEXT",
            "--",
            "list files"
        ])
    );
}

#[test]
fn resume_command_uses_session_and_omits_role_prompt() {
    let req = TurnRequest {
        session_id: Some(SESSION.into()),
        role_prompt: Some("ROLE TEXT".into()),
        ..base()
    };
    assert_eq!(
        Pi.command(&req).args,
        s(&["--mode", "json", "--session", SESSION, "--", "list files"])
    );
}

#[test]
fn extra_args_permission_args_and_env_are_passed_through() {
    let req = TurnRequest {
        env: vec![("WTEAM_AGENT".into(), "worker.1".into())],
        extra_args: s(&["--tools", "read,ls"]),
        permissions: Translation {
            args: s(&["--perm"]),
            env: vec![("PERM_ENV".into(), "1".into())],
            ..Translation::default()
        },
        ..base()
    };
    let spec = Pi.command(&req);
    assert_eq!(
        spec.args,
        s(&[
            "--mode",
            "json",
            "--perm",
            "--tools",
            "read,ls",
            "--",
            "list files"
        ])
    );
    assert_eq!(
        spec.env,
        vec![
            ("WTEAM_AGENT".to_string(), "worker.1".to_string()),
            ("PERM_ENV".to_string(), "1".to_string()),
        ]
    );
}

#[test]
fn prompt_starting_with_dash_stays_a_prompt() {
    let req = TurnRequest {
        prompt: "--help".into(),
        ..base()
    };
    let args = Pi.command(&req).args;
    assert_eq!(args[args.len() - 2..], s(&["--", "--help"]));
}

#[test]
fn first_turn_fixture_yields_session_tokens_and_final_text() {
    let out = Pi.parse_output(&fixture("first-turn.jsonl")).unwrap();
    assert_eq!(out.session_id.as_deref(), Some(SESSION));
    // input + cacheRead + cacheWrite of the last assistant message.
    assert_eq!(out.context_tokens, Some(1500));
    // Only the final assistant message; the tool-call turn's text is absent.
    assert_eq!(out.text, "The repo has Cargo.toml and src.");
}

#[test]
fn error_stop_reason_fails_with_message() {
    let err = Pi
        .parse_output(&fixture("error.jsonl"))
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("pi: turn failed: rate limit exceeded"),
        "{err}"
    );
}

#[test]
fn malformed_output_names_cli_and_line() {
    let err = Pi
        .parse_output(&fixture("malformed.jsonl"))
        .unwrap_err()
        .to_string();
    assert!(err.contains("pi"), "{err}");
    assert!(err.contains("line 2"), "{err}");
    assert!(err.contains(r#"{"type":"agent_start""#), "{err}");
}

#[test]
fn output_without_assistant_message_fails() {
    let header = fixture("malformed.jsonl")
        .lines()
        .next()
        .unwrap()
        .to_string();
    let err = Pi.parse_output(&header).unwrap_err().to_string();
    assert!(err.contains("pi"), "{err}");
    assert!(err.contains("no assistant message"), "{err}");
}

#[test]
fn permissions_show_fallback_column_for_pi() {
    let r = Repo::new("demo");
    let out = r
        .wteam()
        .args(["permissions", "--profile", "edit", "--cli", "pi"])
        .assert()
        .success()
        .stdout(contains("pi runs tools unrestricted; use a sandbox"));
    let text = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(text.contains("✘"), "{text}");
    assert!(text.contains("pi"), "{text}");
}

#[test]
fn bypass_profile_is_exact_for_pi() {
    let r = Repo::new("demo");
    let out = r
        .wteam()
        .args(["permissions", "--profile", "full", "--cli", "pi"])
        .assert()
        .success();
    let text = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(
        text.lines()
            .any(|l| l.starts_with("bypass") && l.ends_with('✔')),
        "{text}"
    );
    assert!(!text.contains("pi runs tools unrestricted"), "{text}");
}

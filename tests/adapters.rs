//! Golden tests for the claude, codex and opencode adapters over recorded fixtures.

use std::path::{Path, PathBuf};

use wteam::adapters::{Adapter, TurnRequest, claude::Claude, codex::Codex, opencode::Opencode};
use wteam::permissions::Translation;

fn fixture(rel: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/adapters")
        .join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

fn pair(k: &str, v: &str) -> (String, String) {
    (k.into(), v.into())
}

fn first() -> TurnRequest {
    TurnRequest {
        agent_id: "worker.1".into(),
        model: Some("m1".into()),
        session_id: None,
        prompt: "do it".into(),
        role_prompt: Some("ROLE".into()),
        cwd: PathBuf::from("/work"),
        env: vec![pair("WTEAM_AGENT", "worker.1")],
        extra_args: s(&["--extra"]),
        permissions: Translation {
            args: s(&["--perm", "x"]),
            env: vec![pair("PERM_ENV", "1")],
            ..Default::default()
        },
    }
}

fn resume() -> TurnRequest {
    TurnRequest {
        session_id: Some("sess-9".into()),
        ..first()
    }
}

// claude

#[test]
fn claude_first_turn_command() {
    let c = Claude.command_for(&first(), false);
    assert_eq!(c.program, "claude");
    assert_eq!(
        c.args,
        s(&[
            "-p",
            "do it",
            "--output-format",
            "stream-json",
            "--verbose",
            "--setting-sources",
            "project,local",
            "--disable-slash-commands",
            "--model",
            "m1",
            "--append-system-prompt",
            "ROLE",
            "--perm",
            "x",
            "--extra"
        ])
    );
    assert_eq!(
        c.env,
        vec![pair("WTEAM_AGENT", "worker.1"), pair("PERM_ENV", "1")]
    );
    assert_eq!(c.cwd, PathBuf::from("/work"));
    assert_eq!(c.stdin, None);
}

#[test]
fn claude_resume_command_omits_role_prompt() {
    let c = Claude.command_for(&resume(), false);
    assert_eq!(
        c.args,
        s(&[
            "-p",
            "do it",
            "--output-format",
            "stream-json",
            "--verbose",
            "--setting-sources",
            "project,local",
            "--disable-slash-commands",
            "--model",
            "m1",
            "--resume",
            "sess-9",
            "--perm",
            "x",
            "--extra"
        ])
    );
}

/// `WTEAM_CLAUDE_USER_SETTINGS=1` drops both isolation flags. `command` reads that variable.
#[test]
fn claude_user_settings_opt_out_omits_isolation_flags() {
    let c = Claude.command_for(&first(), true);
    assert_eq!(
        c.args,
        s(&[
            "-p",
            "do it",
            "--output-format",
            "stream-json",
            "--verbose",
            "--model",
            "m1",
            "--append-system-prompt",
            "ROLE",
            "--perm",
            "x",
            "--extra"
        ])
    );
}

#[test]
fn claude_parse_fixture() {
    let o = Claude
        .parse_output(&fixture("claude/first-turn.jsonl"))
        .unwrap();
    assert_eq!(
        o.session_id.as_deref(),
        Some("785b211d-65ed-4b25-977f-a59b010c2c9d")
    );
    // 2 input + 26359 cache read + 1005 cache creation, from the last assistant event.
    assert_eq!(o.context_tokens, Some(27366));
    assert_eq!(o.text, "ok");
}

#[test]
fn claude_malformed_names_cli_and_line() {
    let e = Claude
        .parse_output(&fixture("claude/malformed.jsonl"))
        .unwrap_err()
        .to_string();
    assert!(
        e.contains("claude") && e.contains("line 2") && e.contains("this is not json"),
        "{e}"
    );
}

#[test]
fn claude_error_result_and_missing_result_fail() {
    let e = Claude
        .parse_output(&fixture("claude/error.jsonl"))
        .unwrap_err()
        .to_string();
    assert!(e.contains("claude") && e.contains("boom"), "{e}");
    assert!(Claude.parse_output("").is_err());
}

// codex

#[test]
fn codex_first_turn_command() {
    let c = Codex.command(&first());
    assert_eq!(c.program, "codex");
    assert_eq!(
        c.args,
        s(&[
            "exec",
            "--json",
            "-m",
            "m1",
            "--perm",
            "x",
            "--extra",
            "ROLE\n\ndo it"
        ])
    );
    assert_eq!(
        c.env,
        vec![pair("WTEAM_AGENT", "worker.1"), pair("PERM_ENV", "1")]
    );
    assert_eq!(c.stdin, Some(String::new()));
}

#[test]
fn codex_resume_command() {
    let c = Codex.command(&resume());
    assert_eq!(
        c.args,
        s(&[
            "exec", "resume", "--json", "--perm", "x", "--extra", "sess-9", "do it"
        ])
    );
    assert_eq!(c.stdin, Some(String::new()));
}

#[test]
fn codex_parse_fixture() {
    let o = Codex
        .parse_output(&fixture("codex/first-turn.jsonl"))
        .unwrap();
    assert_eq!(
        o.session_id.as_deref(),
        Some("01a161bc-1eb6-7643-8769-56cc201d03b5")
    );
    assert_eq!(o.context_tokens, None);
    assert_eq!(o.text, "ok");
}

#[test]
fn codex_usage_limit_fails_with_message() {
    let e = Codex
        .parse_output(&fixture("codex/usage-limit.jsonl"))
        .unwrap_err()
        .to_string();
    assert!(e.contains("codex") && e.contains("usage limit"), "{e}");
}

#[test]
fn codex_malformed_names_cli_and_line() {
    let e = Codex
        .parse_output(&fixture("codex/malformed.jsonl"))
        .unwrap_err()
        .to_string();
    assert!(
        e.contains("codex")
            && e.contains("line 2")
            && e.contains(r#"{"type":"item.completed","item":"#),
        "{e}"
    );
}

/// Both `$CODEX_HOME` cases share one test: the environment is process-global.
#[test]
fn codex_fallback_reads_newest_rollout_under_codex_home() {
    let home = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/adapters/codex/home");
    // SAFETY: no other test in this binary reads or writes CODEX_HOME.
    unsafe { std::env::set_var("CODEX_HOME", &home) };
    let id = "01a161bc-1eb6-7643-8769-56cc201d03b5";
    // The 10/21 rollout's last token_count wins over its null-info line and the older file.
    assert_eq!(Codex.context_tokens_fallback(id), Some(17541));
    assert_eq!(Codex.context_tokens_fallback("unknown-id"), None);
}

// opencode

#[test]
fn opencode_first_turn_command() {
    let c = Opencode.command(&first());
    assert_eq!(c.program, "opencode");
    assert_eq!(
        c.args,
        s(&[
            "run",
            "--format",
            "json",
            "-m",
            "m1",
            "--perm",
            "x",
            "--extra",
            "ROLE\n\ndo it"
        ])
    );
    assert_eq!(
        c.env,
        vec![pair("WTEAM_AGENT", "worker.1"), pair("PERM_ENV", "1")]
    );
    assert_eq!(c.stdin, None);
}

#[test]
fn opencode_resume_command() {
    let c = Opencode.command(&resume());
    assert_eq!(
        c.args,
        s(&[
            "run", "--format", "json", "-m", "m1", "-s", "sess-9", "--perm", "x", "--extra",
            "do it"
        ])
    );
}

#[test]
fn opencode_parse_fixture() {
    let o = Opencode
        .parse_output(&fixture("opencode/first-turn.jsonl"))
        .unwrap();
    assert_eq!(o.session_id.as_deref(), Some("ses_1a2b3c"));
    // Last step_finish: 1200 input + 300 cache read.
    assert_eq!(o.context_tokens, Some(1500));
    // Only the last message's text parts, concatenated.
    assert_eq!(o.text, "all done");
}

#[test]
fn opencode_malformed_names_cli_and_line() {
    let e = Opencode
        .parse_output(&fixture("opencode/malformed.jsonl"))
        .unwrap_err()
        .to_string();
    assert!(
        e.contains("opencode") && e.contains("line 2") && e.contains("provider not configured"),
        "{e}"
    );
}

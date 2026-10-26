//! The cursor adapter: headless `agent -p` with stream-json output.

use anyhow::{Result, bail};
use serde_json::Value;

use super::{Adapter, CommandSpec, TurnOutput, TurnRequest};

pub struct Cursor;

/// Override the binary (default `agent`). Useful when `cursor-agent` is preferred.
pub const BIN_ENV: &str = "WTEAM_CURSOR_BIN";

impl Adapter for Cursor {
    fn name(&self) -> &'static str {
        "cursor"
    }

    fn command(&self, req: &TurnRequest) -> CommandSpec {
        let mut args: Vec<String> = vec![
            "-p".into(),
            "--output-format".into(),
            "stream-json".into(),
            "--trust".into(),
        ];
        if let Some(model) = &req.model {
            args.extend(["--model".into(), model.clone()]);
        }
        if let Some(id) = &req.session_id {
            args.extend(["--resume".into(), id.clone()]);
        }
        // Workspace is the turn cwd (main repo or a worktree). Permissions may add `--force`
        // (bypass profiles) or write `.cursor/cli.json`.
        args.extend(["--workspace".into(), req.cwd.display().to_string()]);
        args.extend(req.permissions.args.iter().cloned());
        args.extend(req.extra_args.iter().cloned());
        let prompt = match (&req.session_id, &req.role_prompt) {
            (None, Some(role)) => format!("{role}\n\n{}", req.prompt),
            _ => req.prompt.clone(),
        };
        args.push(prompt);
        let mut env = req.env.clone();
        env.extend(req.permissions.env.iter().cloned());
        let program = std::env::var(BIN_ENV).unwrap_or_else(|_| "agent".into());
        CommandSpec {
            program,
            args,
            env,
            cwd: req.cwd.clone(),
            stdin: None,
        }
    }

    /// Session from any event; context from the `result` usage; text from `result`.
    fn parse_output(&self, stdout: &str) -> Result<TurnOutput> {
        let mut out = TurnOutput::default();
        let mut result: Option<String> = None;
        for (i, line) in stdout.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let Ok(ev) = serde_json::from_str::<Value>(line) else {
                bail!("cursor: malformed output at line {}: {line}", i + 1);
            };
            if let Some(id) = ev["session_id"].as_str() {
                out.session_id = Some(id.to_string());
            }
            if let Some("result") = ev["type"].as_str() {
                let text = ev["result"].as_str().unwrap_or_default().to_string();
                if ev["is_error"].as_bool().unwrap_or(false) {
                    bail!("cursor: turn failed: {text}");
                }
                let u = &ev["usage"];
                if u.is_object() {
                    let n = |k: &str| u[k].as_u64().unwrap_or(0);
                    // camelCase from agent CLI; treat input+cache as context in use.
                    out.context_tokens =
                        Some(n("inputTokens") + n("cacheReadTokens") + n("cacheWriteTokens"));
                }
                result = Some(text);
            }
        }
        match result {
            Some(text) => {
                out.text = text;
                Ok(out)
            }
            None => bail!("cursor: output has no result event"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::TurnRequest;
    use crate::permissions::Translation;
    use std::path::PathBuf;

    #[test]
    fn first_turn_prepends_role_and_trusts_workspace() {
        let req = TurnRequest {
            prompt: "do the thing".into(),
            role_prompt: Some("you are worker".into()),
            model: Some("composer-2.5".into()),
            cwd: PathBuf::from("/repo"),
            permissions: Translation {
                args: vec!["--force".into()],
                ..Translation::default()
            },
            ..TurnRequest::default()
        };
        let spec = Cursor.command(&req);
        assert_eq!(spec.program, "agent");
        assert!(
            spec.args
                .windows(2)
                .any(|w| w == ["--model", "composer-2.5"])
        );
        assert!(spec.args.windows(2).any(|w| w == ["--workspace", "/repo"]));
        assert!(spec.args.iter().any(|a| a == "--trust"));
        assert!(spec.args.iter().any(|a| a == "--force"));
        assert_eq!(
            spec.args.last().map(String::as_str),
            Some("you are worker\n\ndo the thing")
        );
        assert!(!spec.args.iter().any(|a| a == "--resume"));
    }

    #[test]
    fn resume_skips_role_prompt() {
        let req = TurnRequest {
            prompt: "continue".into(),
            role_prompt: Some("ignored".into()),
            session_id: Some("abc".into()),
            cwd: PathBuf::from("/wt"),
            ..TurnRequest::default()
        };
        let spec = Cursor.command(&req);
        assert!(spec.args.windows(2).any(|w| w == ["--resume", "abc"]));
        assert_eq!(spec.args.last().map(String::as_str), Some("continue"));
    }

    #[test]
    fn parse_stream_json_result() {
        let stdout = r#"
{"type":"system","subtype":"init","session_id":"s1"}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"hi"}]},"session_id":"s1"}
{"type":"result","subtype":"success","is_error":false,"result":"done","session_id":"s1","usage":{"inputTokens":10,"outputTokens":2,"cacheReadTokens":5,"cacheWriteTokens":1}}
"#;
        let out = Cursor.parse_output(stdout).unwrap();
        assert_eq!(out.session_id.as_deref(), Some("s1"));
        assert_eq!(out.text, "done");
        assert_eq!(out.context_tokens, Some(16));
    }

    #[test]
    fn parse_error_result() {
        let stdout = r#"{"type":"result","subtype":"error","is_error":true,"result":"boom","session_id":"s1"}"#;
        let err = Cursor.parse_output(stdout).unwrap_err().to_string();
        assert!(err.contains("boom"), "{err}");
    }
}

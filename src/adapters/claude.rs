//! The claude adapter: `claude -p` with stream-json output.

use anyhow::{Result, bail};
use serde_json::Value;

use super::{Adapter, CommandSpec, TurnOutput, TurnRequest};

pub struct Claude;

impl Adapter for Claude {
    fn name(&self) -> &'static str {
        "claude"
    }

    fn command(&self, req: &TurnRequest) -> CommandSpec {
        let mut args: Vec<String> = vec![
            "-p".into(),
            req.prompt.clone(),
            "--output-format".into(),
            "stream-json".into(),
            "--verbose".into(),
        ];
        if let Some(model) = &req.model {
            args.extend(["--model".into(), model.clone()]);
        }
        match (&req.session_id, &req.role_prompt) {
            (Some(id), _) => args.extend(["--resume".into(), id.clone()]),
            (None, Some(role)) => args.extend(["--append-system-prompt".into(), role.clone()]),
            (None, None) => {}
        }
        args.extend(req.permissions.args.iter().cloned());
        args.extend(req.extra_args.iter().cloned());
        let mut env = req.env.clone();
        env.extend(req.permissions.env.iter().cloned());
        CommandSpec {
            program: "claude".into(),
            args,
            env,
            cwd: req.cwd.clone(),
            stdin: None,
        }
    }

    /// Session id from any event, context from the last top-level `assistant`
    /// usage, text from the `result` event.
    fn parse_output(&self, stdout: &str) -> Result<TurnOutput> {
        let mut out = TurnOutput::default();
        let mut result: Option<String> = None;
        for (i, line) in stdout.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let Ok(ev) = serde_json::from_str::<Value>(line) else {
                bail!("claude: malformed output at line {}: {line}", i + 1);
            };
            if let Some(id) = ev["session_id"].as_str() {
                out.session_id = Some(id.to_string());
            }
            match ev["type"].as_str() {
                Some("assistant") if ev["parent_tool_use_id"].is_null() => {
                    let u = &ev["message"]["usage"];
                    if u.is_object() {
                        let n = |k: &str| u[k].as_u64().unwrap_or(0);
                        out.context_tokens = Some(
                            n("input_tokens")
                                + n("cache_read_input_tokens")
                                + n("cache_creation_input_tokens"),
                        );
                    }
                }
                Some("result") => {
                    let text = ev["result"].as_str().unwrap_or_default().to_string();
                    if ev["is_error"].as_bool().unwrap_or(false) {
                        bail!("claude: turn failed: {text}");
                    }
                    result = Some(text);
                }
                _ => {}
            }
        }
        match result {
            Some(text) => {
                out.text = text;
                Ok(out)
            }
            None => bail!("claude: output has no result event"),
        }
    }
}

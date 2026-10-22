//! The pi adapter: `pi --mode json` (`@earendil-works/pi-coding-agent`).
//!
//! Sources: github.com/badlogic/pi-mono at 58c0b02, `packages/coding-agent/docs/{cli,json,message-types,session-format}.md`.
//! Fixtures under `tests/fixtures/adapters/pi/` are constructed from those docs.

use anyhow::{Result, bail};
use serde_json::Value;

use super::{Adapter, CommandSpec, TurnOutput, TurnRequest};

pub struct Pi;

impl Adapter for Pi {
    fn name(&self) -> &'static str {
        "pi"
    }

    /// `pi --mode json [--model M] [--session ID | --append-system-prompt ROLE] [perm] [extra] -- PROMPT`.
    /// The role prompt rides on the first turn only: a resumed session keeps it.
    fn command(&self, req: &TurnRequest) -> CommandSpec {
        let mut args: Vec<String> = vec!["--mode".into(), "json".into()];
        if let Some(model) = &req.model {
            args.extend(["--model".into(), model.clone()]);
        }
        match (&req.session_id, &req.role_prompt) {
            (Some(id), _) => args.extend(["--session".into(), id.clone()]),
            (None, Some(role)) => args.extend(["--append-system-prompt".into(), role.clone()]),
            (None, None) => {}
        }
        args.extend(req.permissions.args.iter().cloned());
        args.extend(req.extra_args.iter().cloned());
        // `--` keeps a prompt that starts with `-` from being parsed as a flag.
        args.push("--".into());
        args.push(req.prompt.clone());
        let mut env = req.env.clone();
        env.extend(req.permissions.env.iter().cloned());
        CommandSpec {
            program: "pi".into(),
            args,
            env,
            cwd: req.cwd.clone(),
            stdin: None,
        }
    }

    /// Session from the header's `id`; tokens and text from the last `message_end`
    /// whose message is an assistant message. A terminal `error` or `aborted`
    /// stop reason fails the turn.
    fn parse_output(&self, stdout: &str) -> Result<TurnOutput> {
        let mut out = TurnOutput::default();
        let mut last_assistant: Option<Value> = None;
        for (i, line) in stdout.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let Ok(ev) = serde_json::from_str::<Value>(line) else {
                bail!("pi: malformed output at line {}: {line}", i + 1);
            };
            match ev["type"].as_str() {
                Some("session") => {
                    if let Some(id) = ev["id"].as_str() {
                        out.session_id = Some(id.to_string());
                    }
                }
                Some("message_end") if ev["message"]["role"] == "assistant" => {
                    last_assistant = Some(ev["message"].clone());
                }
                _ => {}
            }
        }
        let Some(msg) = last_assistant else {
            bail!("pi: output has no assistant message");
        };
        if matches!(msg["stopReason"].as_str(), Some("error" | "aborted")) {
            let why = msg["errorMessage"].as_str().unwrap_or("no error message");
            bail!("pi: turn failed: {why}");
        }
        let n = |v: &Value| v.as_u64().unwrap_or(0);
        let u = &msg["usage"];
        out.context_tokens = Some(n(&u["input"]) + n(&u["cacheRead"]) + n(&u["cacheWrite"]));
        out.text = msg["content"]
            .as_array()
            .map(|blocks| {
                blocks
                    .iter()
                    .filter(|b| b["type"] == "text")
                    .filter_map(|b| b["text"].as_str())
                    .collect::<String>()
            })
            .unwrap_or_default();
        Ok(out)
    }
}

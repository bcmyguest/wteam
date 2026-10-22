//! The opencode adapter: `opencode run --format json`.

use anyhow::{Result, bail};
use serde_json::Value;

use super::{Adapter, CommandSpec, TurnOutput, TurnRequest};

pub struct Opencode;

impl Adapter for Opencode {
    fn name(&self) -> &'static str {
        "opencode"
    }

    fn command(&self, req: &TurnRequest) -> CommandSpec {
        let mut args: Vec<String> = vec!["run".into(), "--format".into(), "json".into()];
        if let Some(model) = &req.model {
            args.extend(["-m".into(), model.clone()]);
        }
        if let Some(id) = &req.session_id {
            args.extend(["-s".into(), id.clone()]);
        }
        args.extend(req.permissions.args.iter().cloned());
        args.extend(req.extra_args.iter().cloned());
        let prompt = match (&req.session_id, &req.role_prompt) {
            (None, Some(role)) => format!("{role}\n\n{}", req.prompt),
            _ => req.prompt.clone(),
        };
        args.push(prompt);
        let mut env = req.env.clone();
        env.extend(req.permissions.env.iter().cloned());
        CommandSpec {
            program: "opencode".into(),
            args,
            env,
            cwd: req.cwd.clone(),
            stdin: None,
        }
    }

    /// Session from `sessionID`, context from the last `step_finish`
    /// (`tokens.input + tokens.cache.read`), text from the `text` parts of the
    /// last message that has any.
    fn parse_output(&self, stdout: &str) -> Result<TurnOutput> {
        let mut out = TurnOutput::default();
        // (message id, text) per text part, in arrival order.
        let mut texts: Vec<(String, String)> = Vec::new();
        for (i, line) in stdout.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let Ok(ev) = serde_json::from_str::<Value>(line) else {
                bail!("opencode: malformed output at line {}: {line}", i + 1);
            };
            if out.session_id.is_none()
                && let Some(id) = ev["sessionID"].as_str()
            {
                out.session_id = Some(id.to_string());
            }
            let part = &ev["part"];
            match ev["type"].as_str() {
                Some("text") => {
                    if let Some(t) = part["text"].as_str() {
                        let msg = part["messageID"].as_str().unwrap_or_default();
                        texts.push((msg.to_string(), t.to_string()));
                    }
                }
                Some("step_finish") => {
                    let t = &part["tokens"];
                    let n = |v: &Value| v.as_u64().unwrap_or(0);
                    out.context_tokens = Some(n(&t["input"]) + n(&t["cache"]["read"]));
                }
                _ => {}
            }
        }
        if let Some((last, _)) = texts.last() {
            let last = last.clone();
            out.text = texts
                .iter()
                .filter(|(m, _)| *m == last)
                .map(|(_, t)| t.as_str())
                .collect::<String>()
                .trim()
                .to_string();
        }
        Ok(out)
    }
}

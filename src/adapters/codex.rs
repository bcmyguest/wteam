//! The codex adapter: `codex exec --json`.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use serde_json::Value;

use super::{Adapter, CommandSpec, TurnOutput, TurnRequest};

pub struct Codex;

impl Adapter for Codex {
    fn name(&self) -> &'static str {
        "codex"
    }

    fn command(&self, req: &TurnRequest) -> CommandSpec {
        let mut args: Vec<String> = vec!["exec".into()];
        if let Some(id) = &req.session_id {
            args.extend(["resume".into(), "--json".into()]);
            args.extend(req.permissions.args.iter().cloned());
            args.extend(req.extra_args.iter().cloned());
            args.extend([id.clone(), req.prompt.clone()]);
        } else {
            args.push("--json".into());
            if let Some(model) = &req.model {
                args.extend(["-m".into(), model.clone()]);
            }
            args.extend(req.permissions.args.iter().cloned());
            args.extend(req.extra_args.iter().cloned());
            args.push(match &req.role_prompt {
                Some(role) => format!("{role}\n\n{}", req.prompt),
                None => req.prompt.clone(),
            });
        }
        let mut env = req.env.clone();
        env.extend(req.permissions.env.iter().cloned());
        CommandSpec {
            program: "codex".into(),
            args,
            env,
            cwd: req.cwd.clone(),
            stdin: Some(String::new()),
        }
    }

    /// Session from `thread.started`, text from the last `agent_message` item.
    /// `turn.completed.usage` is cumulative, so context comes from the fallback.
    fn parse_output(&self, stdout: &str) -> Result<TurnOutput> {
        let mut out = TurnOutput::default();
        for (i, line) in stdout.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let Ok(ev) = serde_json::from_str::<Value>(line) else {
                bail!("codex: malformed output at line {}: {line}", i + 1);
            };
            match ev["type"].as_str() {
                Some("thread.started") => {
                    out.session_id = ev["thread_id"].as_str().map(str::to_string);
                }
                Some("item.completed") if ev["item"]["type"] == "agent_message" => {
                    if let Some(t) = ev["item"]["text"].as_str() {
                        out.text = t.trim().to_string();
                    }
                }
                Some("turn.failed") => {
                    let msg = ev["error"]["message"].as_str().unwrap_or("unknown error");
                    bail!("codex: turn failed: {msg}");
                }
                _ => {}
            }
        }
        Ok(out)
    }

    /// Last `token_count` in the newest `rollout-*-<id>.jsonl` under
    /// `$CODEX_HOME/sessions` (default `~/.codex`).
    fn context_tokens_fallback(&self, session_id: &str) -> Option<u64> {
        let home = std::env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".codex")))?;
        let suffix = format!("-{session_id}.jsonl");
        let mut newest: Option<(String, PathBuf)> = None;
        find_rollouts(&home.join("sessions"), &suffix, &mut newest);
        let (_, path) = newest?;
        let body = fs::read_to_string(path).ok()?;
        body.lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .filter(|ev| ev["payload"]["type"] == "token_count")
            .filter_map(|ev| ev["payload"]["info"]["last_token_usage"]["input_tokens"].as_u64())
            .next_back()
    }
}

/// Track the `rollout-*<suffix>` file under `dir` with the greatest name; names embed a timestamp.
fn find_rollouts(dir: &Path, suffix: &str, newest: &mut Option<(String, PathBuf)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            find_rollouts(&path, suffix, newest);
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if !(name.starts_with("rollout-") && name.ends_with(suffix)) {
            continue;
        }
        if newest.as_ref().is_none_or(|(n, _)| name > *n) {
            *newest = Some((name, path));
        }
    }
}

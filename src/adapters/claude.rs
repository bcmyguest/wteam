//! The claude adapter: `claude -p` with stream-json output.

use anyhow::{Result, bail};
use serde_json::Value;

use super::{Adapter, CommandSpec, TurnOutput, TurnRequest};

pub struct Claude;

/// Set to `1` to let agents read the user's Claude settings, hooks and skills.
pub const USER_SETTINGS_ENV: &str = "WTEAM_CLAUDE_USER_SETTINGS";

/// True only when the `WTEAM_CLAUDE_USER_SETTINGS` value is exactly `1`.
pub fn user_settings_from(value: Option<&str>) -> bool {
    value == Some("1")
}

impl Claude {
    /// The command for one turn. `user_settings` keeps the user's config: when false, agents
    /// get `--setting-sources project,local` (no user hooks or permission rules) and
    /// `--disable-slash-commands` (no user skills). Both flags are in `claude --help` (2.1.307).
    pub fn command_for(&self, req: &TurnRequest, user_settings: bool) -> CommandSpec {
        let mut args: Vec<String> = vec![
            "-p".into(),
            req.prompt.clone(),
            "--output-format".into(),
            "stream-json".into(),
            "--verbose".into(),
        ];
        if !user_settings {
            args.extend([
                "--setting-sources".into(),
                "project,local".into(),
                "--disable-slash-commands".into(),
            ]);
        }
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
}

impl Adapter for Claude {
    fn name(&self) -> &'static str {
        "claude"
    }

    fn command(&self, req: &TurnRequest) -> CommandSpec {
        let value = std::env::var(USER_SETTINGS_ENV).ok();
        self.command_for(req, user_settings_from(value.as_deref()))
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

#[cfg(test)]
mod tests {
    use super::user_settings_from;

    #[test]
    fn only_exact_one_enables_user_settings() {
        assert!(user_settings_from(Some("1")));
        assert!(!user_settings_from(None));
        assert!(!user_settings_from(Some("")));
        assert!(!user_settings_from(Some("0")));
        assert!(!user_settings_from(Some("true")));
        assert!(!user_settings_from(Some("1 ")));
    }
}

//! Adapters: one per agent CLI. An adapter builds the command for one turn and
//! parses that turn's output; it never runs anything itself.

pub mod claude;
pub mod codex;
pub mod cursor;
pub mod fake;
pub mod opencode;
pub mod pi;

use std::path::PathBuf;

use anyhow::{Result, bail};
use serde::Serialize;

use crate::permissions::Translation;

/// Everything needed to start or resume one agent turn.
#[derive(Debug, Clone, Default)]
pub struct TurnRequest {
    /// Instance id, e.g. `worker.1`.
    pub agent_id: String,
    pub model: Option<String>,
    /// `Some` resumes that session; `None` starts a new one.
    pub session_id: Option<String>,
    /// The message for this turn.
    pub prompt: String,
    /// Role prompt; adapters place it as a system prompt or prepend it on the first turn.
    pub role_prompt: Option<String>,
    pub cwd: PathBuf,
    /// `WTEAM_*` variables the agent's own `wteam` calls rely on.
    pub env: Vec<(String, String)>,
    pub extra_args: Vec<String>,
    pub permissions: Translation,
}

/// A process to run. `stdin`, when set, is written then closed.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub cwd: PathBuf,
    pub stdin: Option<String>,
}

/// What a turn produced.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct TurnOutput {
    pub session_id: Option<String>,
    /// Context in use after the turn, when the CLI reports it.
    pub context_tokens: Option<u64>,
    /// Final assistant text.
    pub text: String,
}

pub trait Adapter: Send + Sync {
    fn name(&self) -> &'static str;
    fn command(&self, req: &TurnRequest) -> CommandSpec;
    fn parse_output(&self, stdout: &str) -> Result<TurnOutput>;
    /// Context tokens when the CLI's stdout lacks them (codex reads its rollout file).
    fn context_tokens_fallback(&self, session_id: &str) -> Option<u64> {
        let _ = session_id;
        None
    }
}

/// The adapter for a CLI name.
pub fn for_cli(cli: &str) -> Result<Box<dyn Adapter>> {
    Ok(match cli {
        "claude" => Box::new(claude::Claude),
        "codex" => Box::new(codex::Codex),
        "cursor" => Box::new(cursor::Cursor),
        "opencode" => Box::new(opencode::Opencode),
        "pi" => Box::new(pi::Pi),
        "fake" => Box::new(fake::Fake),
        other => {
            bail!(
                "unknown agent cli `{other}` (expected claude, codex, opencode, pi, cursor or fake)"
            )
        }
    })
}

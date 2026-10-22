//! `wteam run`.

use anyhow::{Result, bail};
use serde_json::json;

use super::Out;
use crate::cli::{Global, RunArgs};
use crate::engine::Ctx;
use crate::state::{self, RunSettings};
use crate::watcher::{self, RunOptions};

pub fn run(g: &Global, a: &RunArgs) -> Result<Out> {
    let paths = super::effort(g)?;
    match a.isolation.as_str() {
        "none" => {}
        "worktree" => bail!("--isolation worktree is not available yet (ticket 14)"),
        other => bail!("unknown isolation `{other}` (expected none or worktree)"),
    }
    let transport = a.transport.clone().unwrap_or_else(|| "resume".to_string());
    match transport.as_str() {
        "resume" => {}
        "herdr" => bail!("--transport herdr is not available yet (ticket 13)"),
        other => bail!("unknown transport `{other}` (expected resume or herdr)"),
    }
    if a.pairs == 0 {
        bail!("--pairs must be at least 1");
    }
    // Settings first: `Ctx::load` resolves the team from them. `active` stays set until the
    // run ends by any path, so mail to the orchestrator waits for the watcher.
    state::with_state(&paths, |s| {
        s.run = RunSettings {
            pairs: a.pairs,
            isolation: a.isolation.clone(),
            transport: transport.clone(),
            team: a.team.clone(),
            budget_tokens: a.budget_tokens,
            active: true,
            watcher_pid: Some(std::process::id()),
        };
        Ok(())
    })?;
    let result = Ctx::load(paths.clone()).and_then(|ctx| {
        watcher::run(
            &ctx,
            &RunOptions {
                timeout_secs: a.timeout,
                max_turns: a.max_turns,
            },
        )
    });
    state::with_state(&paths, |s| {
        s.run.active = false;
        Ok(())
    })?;
    let report = result?;
    let value = json!({ "report": &report });
    Ok(Out::new(report, value))
}

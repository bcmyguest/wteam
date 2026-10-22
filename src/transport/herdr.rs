//! The herdr transport: agents live in herdr panes; delivery is
//! `herdr agent prompt <name> "<nudge>"`. Requires herdr >= 0.9.5.
//!
//! Flags come from `herdr agent --help`, `herdr agent start --help` and
//! `herdr agent prompt --help` (herdr 0.9.5). Control commands run only inside a herdr pane
//! (`HERDR_ENV=1`). `herdr agent start` needs an existing shell pane, named by
//! `WTEAM_HERDR_PANE`, because it never creates or splits panes.

use std::process::Output;

use anyhow::{Context, Result, bail};

use super::Transport;
use crate::config;
use crate::engine::Ctx;
use crate::team::{NodeKind, agents, parse_instance};

/// Env var naming the pane `herdr agent start` should use for a new agent.
pub const PANE_ENV: &str = "WTEAM_HERDR_PANE";

pub struct Herdr;

impl Transport for Herdr {
    fn name(&self) -> &'static str {
        "herdr"
    }

    fn deliver(&self, ctx: &Ctx, agent: &str) -> Result<()> {
        let kind = herdr_kind(&cli_of(ctx, agent)?)?;
        let name = agent_name(&ctx.paths.slug, agent);
        if !agent_exists(&name)? {
            let pane = std::env::var(PANE_ENV).with_context(|| {
                format!("herdr agent start needs an available shell pane: set {PANE_ENV} to its id")
            })?;
            let args = start_args(&name, kind, &pane);
            check(&args, run_herdr(&args)?)?;
        }
        let nudge = nudge(&ctx.paths.root, &ctx.paths.slug, agent);
        let args = prompt_args(&name, &nudge);
        check(&args, run_herdr(&args)?)
    }
}

/// Herdr agent name for `agent` in effort `slug`: `wteam-<slug>-<agent>`.
pub fn agent_name(slug: &str, agent: &str) -> String {
    format!("wteam-{slug}-{agent}")
}

/// `herdr --kind` value for a wteam CLI name. Errors for CLIs herdr cannot start.
pub fn herdr_kind(cli: &str) -> Result<&'static str> {
    Ok(match cli {
        "claude" => "claude",
        "codex" => "codex",
        "opencode" => "opencode",
        "pi" => "pi",
        other => {
            bail!("herdr cannot start the `{other}` CLI (supported: claude, codex, opencode, pi)")
        }
    })
}

/// Whether herdr knows agent `name`, via `herdr agent get`.
fn agent_exists(name: &str) -> Result<bool> {
    let out = run_herdr(&get_args(name))?;
    interpret_get(
        out.status.success(),
        &String::from_utf8_lossy(&out.stdout),
        &String::from_utf8_lossy(&out.stderr),
    )
}

/// Read the outcome of `herdr agent get`. Herdr reports errors as JSON
/// `{"error":{"code":..,"message":..}}`. Observed: `server_not_running` (exit 1, here, with no
/// server). Assumed: a missing agent gives a code containing `not_found`, since `--help` does
/// not list the codes and no server was started to check. Any other code is an error.
pub fn interpret_get(ok: bool, stdout: &str, stderr: &str) -> Result<bool> {
    if ok {
        return Ok(true);
    }
    let error = [stdout, stderr].iter().find_map(|text| {
        serde_json::from_str::<serde_json::Value>(text.trim())
            .ok()
            .and_then(|v| v.get("error").cloned())
    });
    let Some(error) = error else {
        bail!("herdr agent get failed: {}", stderr.trim());
    };
    let code = error["code"].as_str().unwrap_or_default();
    let message = error["message"].as_str().unwrap_or_default();
    if code.contains("not_found") {
        return Ok(false);
    }
    if code == "server_not_running" {
        bail!("herdr server is not running: start herdr (or run inside a herdr pane)");
    }
    bail!("herdr agent get failed ({code}): {message}")
}

/// `herdr agent get <name>`: exit 0 when the agent exists.
pub fn get_args(name: &str) -> Vec<String> {
    vec!["agent".into(), "get".into(), name.into()]
}

/// `herdr agent start <name> --kind <kind> --pane <pane>`.
pub fn start_args(name: &str, kind: &str, pane: &str) -> Vec<String> {
    ["agent", "start", name, "--kind", kind, "--pane", pane]
        .map(String::from)
        .to_vec()
}

/// `herdr agent prompt <name> <nudge>`.
pub fn prompt_args(name: &str, nudge: &str) -> Vec<String> {
    vec!["agent".into(), "prompt".into(), name.into(), nudge.into()]
}

/// The text herdr types into the agent's pane. It names the agent's inbox command.
pub fn nudge(root: &std::path::Path, slug: &str, agent: &str) -> String {
    format!(
        "New wteam mail. Run: wteam --root {} --slug {} --agent {} inbox",
        shell_quote(&root.display().to_string()),
        shell_quote(slug),
        shell_quote(agent)
    )
}

/// POSIX single-quote `text` so the agent's shell reads it as one word.
pub fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// Run `herdr <args>`; a missing binary is an error, a non-zero exit is the caller's to judge.
fn run_herdr(args: &[String]) -> Result<Output> {
    std::process::Command::new("herdr")
        .args(args)
        .output()
        .context("running herdr (is it on PATH?)")
}

/// Turn a failed herdr call into an error that names the pane requirement.
fn check(args: &[String], out: Output) -> Result<()> {
    if out.status.success() {
        return Ok(());
    }
    let mut detail = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if detail.is_empty() {
        detail = String::from_utf8_lossy(&out.stdout).trim().to_string();
    }
    bail!(
        "herdr {} failed: {detail} (herdr control commands need a herdr pane: run wteam inside one, HERDR_ENV=1)",
        args.join(" ")
    )
}

/// CLI of the agent definition behind `agent`'s role in the run's team.
fn cli_of(ctx: &Ctx, agent: &str) -> Result<String> {
    let (role, _) = parse_instance(agent);
    let team = crate::state::load(&ctx.paths)?.run.team;
    let dirs = config::search_dirs(&ctx.paths, &team);
    let name = match ctx.team.roles.get(&role).map(|r| &r.kind) {
        Some(NodeKind::Agent { agent, .. }) => agent.clone(),
        _ => bail!("`{role}` is not an agent role in team `{}`", ctx.team.name),
    };
    Ok(agents::load(&name, &dirs.agents)?.cli)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_exact_command_lines() {
        assert_eq!(agent_name("feat", "worker.2"), "wteam-feat-worker.2");
        assert_eq!(
            start_args("wteam-feat-worker.2", "claude", "p_3"),
            [
                "agent",
                "start",
                "wteam-feat-worker.2",
                "--kind",
                "claude",
                "--pane",
                "p_3"
            ]
        );
        assert_eq!(
            prompt_args("wteam-feat-worker.2", "hi"),
            ["agent", "prompt", "wteam-feat-worker.2", "hi"]
        );
        assert_eq!(get_args("n"), ["agent", "get", "n"]);
    }

    #[test]
    fn nudge_names_the_inbox_command() {
        let n = nudge(std::path::Path::new("/repo"), "feat", "worker.1");
        assert!(n.contains("wteam --root '/repo' --slug 'feat' --agent 'worker.1' inbox"));
        let spaced = nudge(std::path::Path::new("/my repo/it's"), "feat", "worker.1");
        assert!(spaced.contains("--root '/my repo/it'\\''s'"), "{spaced}");
    }

    #[test]
    fn get_outcomes() {
        // Found.
        assert!(interpret_get(true, "", "").unwrap());
        // Missing agent (assumed code).
        let missing = r#"{"id":"x","error":{"code":"agent_not_found","message":"no agent"}}"#;
        assert!(!interpret_get(false, missing, "").unwrap());
        // Server down (observed code): a clear message, not "missing".
        let down = r#"{"id":"cli:agent:get","error":{"code":"server_not_running","message":"no herdr server"}}"#;
        let err = interpret_get(false, down, "").unwrap_err().to_string();
        assert_eq!(
            err,
            "herdr server is not running: start herdr (or run inside a herdr pane)"
        );
        // Any other code: herdr's own message.
        let other = r#"{"error":{"code":"agent_blocked","message":"waiting on a dialog"}}"#;
        let err = interpret_get(false, other, "").unwrap_err().to_string();
        assert!(
            err.contains("agent_blocked") && err.contains("waiting on a dialog"),
            "{err}"
        );
    }

    #[test]
    fn maps_supported_clis_only() {
        assert_eq!(herdr_kind("claude").unwrap(), "claude");
        assert_eq!(herdr_kind("codex").unwrap(), "codex");
        assert!(herdr_kind("fake").is_err());
    }
}

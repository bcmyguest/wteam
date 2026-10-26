//! One module per subcommand group. Each returns an [`Out`]: human text plus JSON.

pub mod config;
pub mod graph;
pub mod init;
pub mod mail;
pub mod permissions;
pub mod phase;
pub mod pickup;
pub mod refine;
pub mod report;
pub mod role;
pub mod run;
pub mod setup;
pub mod status;
pub mod stop;
pub mod tickets;
pub mod turn;
pub mod wait;

use anyhow::Result;

use crate::cli::{Command, Global};
use crate::paths::{self, Paths};

/// A command's result: `text` for humans, `json` for `--json`.
#[derive(Debug, Clone)]
pub struct Out {
    pub text: String,
    pub json: serde_json::Value,
}

/// Repo root from `--root`/`WTEAM_ROOT`, else the git toplevel of the cwd.
pub fn root(g: &Global) -> Result<std::path::PathBuf> {
    match &g.root {
        Some(r) => Ok(r.clone()),
        None => Ok(paths::repo_root(&std::env::current_dir()?)),
    }
}

/// Paths for the effort selected by `--slug`/`WTEAM_SLUG` or the only one present.
pub fn effort(g: &Global) -> Result<Paths> {
    let root = root(g)?;
    let slug = paths::resolve_slug(&root, g.slug.as_deref())?;
    Ok(Paths::new(root, slug))
}

/// The calling agent from `--agent`/`WTEAM_AGENT`, or `fallback` for humans.
pub fn caller(g: &Global, fallback: &str) -> String {
    g.agent.clone().unwrap_or_else(|| fallback.to_string())
}

/// Warn when a previous run left tracked files applied and nothing is running.
fn warn_stale_manifest(g: &Global, cmd: &Command) {
    if matches!(cmd, Command::Stop | Command::Restore | Command::Init) {
        return;
    }
    let Ok(paths) = effort(g) else { return };
    if !crate::backup::pending(&paths) {
        return;
    }
    let busy = crate::state::load(&paths).is_ok_and(|s| s.agents.values().any(|a| a.busy));
    if !busy {
        eprintln!("warning: files from an interrupted run are still applied; run `wteam restore`");
    }
}

pub fn run(g: &Global, cmd: &Command) -> Result<Out> {
    warn_stale_manifest(g, cmd);
    match cmd {
        Command::Init => init::run(g),
        Command::Pickup(a) => pickup::run(g, a),
        Command::Phase(a) => phase::phase(g, a),
        Command::PhaseVerdict(a) => phase::phase_verdict(g, a),
        Command::PlanReview(a) => phase::plan_review(g, a),
        Command::PlanVerdict(a) => phase::plan_verdict(g, a),
        Command::Status => status::status(g),
        Command::Ready => status::ready(g),
        Command::Tickets(c) => tickets::run_subcommand(g, c),
        Command::Dispatch(a) => tickets::dispatch(g, a),
        Command::Submit(a) => tickets::submit(g, a),
        Command::Verdict(a) => tickets::verdict(g, a),
        Command::Block(a) => tickets::block(g, a),
        Command::Requeue(a) => tickets::requeue(g, a),
        Command::Approve(a) => tickets::approve(g, a),
        Command::Send(a) => mail::send(g, a),
        Command::Inbox(a) => mail::inbox(g, a),
        Command::Wait(a) => wait::run(g, a),
        Command::Run(a) => run::run(g, a),
        Command::Stop => stop::stop(g),
        Command::Restore => stop::restore(g),
        Command::Report => report::run(g),
        Command::Refine(a) => refine::run(g, a),
        Command::Role(a) => role::run(g, a),
        Command::Graph(a) => graph::run(g, a),
        Command::Permissions(a) => permissions::run(g, a),
        Command::Config(c) => config::run(g, c),
        Command::Setup(a) => setup::run(a),
        Command::Turn(a) => turn::run(g, a),
    }
}

//! wteam: portable worker/reviewer teams for coding agents.
//!
//! Module map (one owner per module; see `.wteam/wteam/architecture.md`):
//! - [`paths`], [`state`], [`mailbox`]: the on-disk `.wteam/` layout and its locked state.
//! - [`work`]: the work graph — tickets in the `/to-tickets` local format.
//! - [`team`]: the team graph (DOT subset) and agent definitions (TOML).
//! - [`permissions`]: Claude-syntax profiles and per-CLI translation.
//! - [`adapters`]: one [`adapters::Adapter`] per agent CLI.
//! - [`transport`]: message delivery ([`transport::Transport`]): `resume`, `herdr`.
//! - [`engine`]: deterministic routing of agent events through the team graph.
//! - [`watcher`]: `wteam run` — drives the orchestrator and its handoffs.
//! - [`cli`] and [`commands`]: argument parsing and one file per subcommand.

pub mod adapters;
pub mod backup;
pub mod cli;
pub mod commands;
pub mod config;
pub mod engine;
pub mod mailbox;
pub mod paths;
pub mod permissions;
pub mod phase;
pub mod pickup;
pub mod plan_review;
pub mod render;
pub mod report;
pub mod state;
pub mod team;
pub mod transport;
pub mod watcher;
pub mod work;
pub mod worktree;

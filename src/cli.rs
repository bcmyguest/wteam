//! Argument parsing and dispatch. Each subcommand's logic lives in `commands/`.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::commands::{self, Out};

#[derive(Debug, Parser)]
#[command(
    name = "wteam",
    version,
    about = "Portable worker/reviewer teams for coding agents"
)]
pub struct Cli {
    #[command(flatten)]
    pub global: Global,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clone, Args)]
pub struct Global {
    /// Repo root (default: git toplevel of the current directory).
    #[arg(long, global = true, env = "WTEAM_ROOT")]
    pub root: Option<PathBuf>,
    /// Effort slug under `.wteam/` (default: the only one).
    #[arg(long, global = true, env = "WTEAM_SLUG")]
    pub slug: Option<String>,
    /// Calling agent instance, e.g. `worker.1` (set for spawned agents).
    #[arg(long, global = true, env = "WTEAM_AGENT")]
    pub agent: Option<String>,
    /// Machine-readable output.
    #[arg(long, global = true)]
    pub json: bool,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create `.wteam/` (self-ignoring) and an effort directory.
    Init,
    /// Import planning material and verify it before a run.
    Pickup(PickupArgs),
    /// Run a Goldfish phase (persona chain; presets: plan-review, validate).
    Phase(PhaseArgs),
    /// Record a Goldfish phase verdict (requires WTEAM_AGENT / WTEAM_PHASE).
    PhaseVerdict(PhaseVerdictArgs),
    /// Alias: `wteam phase plan-review`.
    PlanReview(PlanReviewArgs),
    /// Alias: `wteam phase-verdict` for the plan-review phase.
    PlanVerdict(PlanVerdictArgs),
    /// Tickets, agents and pending events.
    Status,
    /// Tickets whose blockers are done.
    Ready,
    /// Assign a ticket to a worker and start its turn (orchestrator).
    Dispatch(DispatchArgs),
    /// Hand a finished round to review (worker).
    Submit(SubmitArgs),
    /// Record a review verdict (reviewer).
    Verdict(VerdictArgs),
    /// Mark a ticket blocked.
    Block(BlockArgs),
    /// Return a blocked ticket to todo with an optional note (orchestrator).
    Requeue(RequeueArgs),
    /// Pass a human gate.
    Approve(TicketArg),
    /// Message another agent instance.
    Send(SendArgs),
    /// Read your mailbox.
    Inbox(InboxArgs),
    /// Block until orchestrator events arrive (orchestrator).
    Wait(WaitArgs),
    /// Run the team: drive the orchestrator until nothing is runnable.
    Run(RunArgs),
    /// Stop the run, restore backed-up files, print the stop report.
    Stop,
    /// Restore files backed up by an interrupted run.
    Restore,
    /// Print the stop report.
    Report,
    /// Print a resolved role prompt (project → config dir → built-in).
    Role(RoleArgs),
    /// Render the team or work graph as Mermaid.
    Graph(GraphArgs),
    /// Show how permission profiles translate per CLI (✔ exact, ≈ lossy, ✘ unsupported).
    Permissions(PermissionsArgs),
    /// Saved teams under ~/.config/wteam.
    #[command(subcommand)]
    Config(ConfigCommand),
    /// Detect herdr, pick a transport, write ~/.config/wteam/config.toml.
    Setup(SetupArgs),
    /// Internal: run an agent's turns (spawned by the resume transport).
    #[command(name = "__turn", hide = true)]
    Turn(TurnArgs),
}

#[derive(Debug, Args)]
pub struct PickupArgs {
    /// Directory written by /to-tickets (`.scratch/<slug>`); omit to verify `.wteam/<slug>` in place.
    #[arg(long)]
    pub from: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct PhaseArgs {
    /// Phase name: built-in presets `plan-review` and `validate`, or any name with `--persona`.
    pub name: String,
    /// Goldfish agent def / role names, in order (default: preset chain for known names).
    #[arg(long = "persona")]
    pub persona: Vec<String>,
    /// Optional ticket id (validate zoom-local hint).
    #[arg(long)]
    pub ticket: Option<String>,
}

#[derive(Debug, Args)]
pub struct PhaseVerdictArgs {
    pub verdict: VerdictValue,
    /// Phase name (default: `WTEAM_PHASE` from `wteam phase`).
    #[arg(long)]
    pub phase: Option<String>,
    /// Blocking problem (FAIL only); repeatable.
    #[arg(long = "finding")]
    pub findings: Vec<String>,
    /// Non-blocking note; repeatable.
    #[arg(long = "note")]
    pub notes: Vec<String>,
    /// File with `FINDINGS:` / `NOTES:` sections of `- ` bullets; merged with the flags.
    #[arg(long = "file")]
    pub file: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct PlanReviewArgs {
    /// Goldfish agent def / role names, in order (default: built-in spec-* chain).
    #[arg(long = "persona")]
    pub persona: Vec<String>,
}

#[derive(Debug, Args)]
pub struct PlanVerdictArgs {
    pub verdict: VerdictValue,
    #[arg(long = "finding")]
    pub findings: Vec<String>,
    #[arg(long = "note")]
    pub notes: Vec<String>,
    #[arg(long = "file")]
    pub file: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct RoleArgs {
    /// Role name: `planner`, `worker`, or any `roles/<name>.md`.
    pub name: String,
}

#[derive(Debug, Args)]
pub struct TicketArg {
    pub ticket: String,
}

#[derive(Debug, Args)]
pub struct DispatchArgs {
    /// Ticket to dispatch. Omit with `--fill`.
    #[arg(required_unless_present = "fill", conflicts_with = "fill")]
    pub ticket: Option<String>,
    /// Pair index (default: first idle pair).
    #[arg(long)]
    pub pair: Option<u32>,
    /// Dispatch every dispatchable ready ticket to the idle pairs, lowest id first.
    #[arg(long)]
    pub fill: bool,
}

#[derive(Debug, Args)]
pub struct SubmitArgs {
    pub ticket: String,
    /// Summary on the command line; `--summary-file` is safer for free text.
    #[arg(long, default_value = "", conflicts_with = "summary_file")]
    pub summary: String,
    /// File holding the summary, read as-is; relative to the current directory.
    #[arg(long = "summary-file", conflicts_with = "summary")]
    pub summary_file: Option<PathBuf>,
    /// Changed path outside the ticket's `Files:`; repeatable. Committed on PASS.
    #[arg(long = "extra")]
    pub extra: Vec<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum VerdictValue {
    Pass,
    Fail,
}

#[derive(Debug, Args)]
pub struct VerdictArgs {
    pub ticket: String,
    pub verdict: VerdictValue,
    /// Blocking problem (FAIL only); repeatable.
    #[arg(long = "finding")]
    pub findings: Vec<String>,
    /// Non-blocking in-ticket note for the worker; repeatable.
    #[arg(long = "note")]
    pub notes: Vec<String>,
    /// Cross-cutting `key:text` for the planner queue; repeatable.
    #[arg(long = "follow-up")]
    pub follow_ups: Vec<String>,
    /// File with `FINDINGS:`, `NOTES:` and `FOLLOW-UPS:` sections of `- ` bullets; merged with the flags.
    #[arg(long = "file")]
    pub file: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct BlockArgs {
    pub ticket: String,
    /// Why the ticket is blocked.
    #[arg(
        long,
        required_unless_present = "reason_file",
        conflicts_with = "reason_file"
    )]
    pub reason: Option<String>,
    /// File holding the reason, read as-is and trimmed; relative to the current directory.
    #[arg(long = "reason-file", conflicts_with = "reason")]
    pub reason_file: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct RequeueArgs {
    pub ticket: String,
    /// Guidance included in the next dispatch brief.
    #[arg(long)]
    pub note: Option<String>,
    /// File holding the guidance, read as-is; relative to the current directory.
    #[arg(long = "note-file", conflicts_with = "note")]
    pub note_file: Option<PathBuf>,
    /// Also discard the ticket's preserved work: remove its worktree and delete its branch, so
    /// the next dispatch starts from current main.
    #[arg(long)]
    pub fresh: bool,
}

#[derive(Debug, Args)]
pub struct SendArgs {
    pub to: String,
    pub message: String,
}

#[derive(Debug, Args)]
pub struct InboxArgs {
    /// Show the whole mailbox, not just unread messages.
    #[arg(long)]
    pub all: bool,
}

#[derive(Debug, Args)]
pub struct WaitArgs {
    #[arg(long, default_value_t = 3600)]
    pub timeout: u64,
}

#[derive(Debug, Args)]
pub struct RunArgs {
    /// Team: preset name, promoted team, or `.dot` path.
    #[arg(long, default_value = "default")]
    pub team: String,
    #[arg(long, default_value_t = 1)]
    pub pairs: u32,
    /// `none` or `worktree` (default: `worktree` when pairs > 1, else `none`).
    #[arg(long)]
    pub isolation: Option<String>,
    /// Transport (default: from ~/.config/wteam/config.toml, else `resume`).
    #[arg(long)]
    pub transport: Option<String>,
    /// Seconds to wait for agent turns before stopping.
    #[arg(long, default_value_t = 7200)]
    pub timeout: u64,
    #[arg(long, default_value_t = 200)]
    pub max_turns: u32,
    /// Stop the run once the agents' summed context tokens exceed this.
    #[arg(long)]
    pub budget_tokens: Option<u64>,
    /// `auto`: the watcher dispatches ready tickets itself and wakes the orchestrator only for
    /// blocked tickets and human gates. `model`: the orchestrator dispatches every ticket.
    #[arg(long, value_enum, default_value_t = DispatchMode::Auto)]
    pub dispatch: DispatchMode,
    /// Under `auto`: give each blocked ticket one planner turn to rewrite it, then requeue it.
    #[arg(long)]
    pub replan: bool,
    /// `serial`: tickets whose `Files:` overlap run one at a time. `allow`: they run at once
    /// (needs worktree isolation); a landing conflict goes back to its worker.
    #[arg(long, default_value = "serial")]
    pub overlap: String,
}

/// Who dispatches ready tickets during `wteam run`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum DispatchMode {
    /// The watcher dispatches; the orchestrator wakes only for blocks and human gates.
    Auto,
    /// The orchestrator dispatches every ticket.
    Model,
}

impl DispatchMode {
    /// The name stored in run settings and shown in reports.
    pub fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Model => "model",
        }
    }
}

#[derive(Debug, Args)]
pub struct GraphArgs {
    /// Render the work graph instead of the team graph.
    #[arg(long)]
    pub work: bool,
    /// Team to render (default: the effort's team, else `default`).
    #[arg(long)]
    pub team: Option<String>,
}

#[derive(Debug, Args)]
pub struct PermissionsArgs {
    /// Profiles to show (default: every built-in profile).
    #[arg(long = "profile")]
    pub profiles: Vec<String>,
    /// CLIs to show: claude, codex, opencode, cursor, pi (default: the first four).
    #[arg(long = "cli")]
    pub clis: Vec<String>,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Save the effort's team, agents and profiles as a named team.
    Promote { name: String },
    /// List saved teams.
    List,
    /// Show a saved team.
    Show { name: String },
}

#[derive(Debug, Args)]
pub struct SetupArgs {
    /// Accept defaults without prompting.
    #[arg(long)]
    pub yes: bool,
    /// Force a transport instead of detecting.
    #[arg(long)]
    pub transport: Option<String>,
}

#[derive(Debug, Args)]
pub struct TurnArgs {
    pub agent: String,
}

/// Entry point used by `main.rs`.
pub fn main() -> ExitCode {
    let cli = Cli::parse();
    let json = cli.global.json;
    match commands::run(&cli.global, &cli.command) {
        Ok(out) => {
            if json {
                println!("{}", out.json);
            } else if !out.text.is_empty() {
                println!("{}", out.text.trim_end());
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            // A fill that failed midway still reports the dispatches that stand.
            let started = e
                .downcast_ref::<crate::engine::FillError>()
                .map(|f| f.started.clone())
                .unwrap_or_default();
            if json {
                let mut body = serde_json::json!({ "error": format!("{e:#}") });
                if !started.is_empty() {
                    body["dispatched"] = started
                        .iter()
                        .map(|(ticket, agent)| serde_json::json!({ "ticket": ticket, "agent": agent }))
                        .collect();
                }
                println!("{body}");
            } else {
                for (ticket, agent) in &started {
                    println!("dispatched {ticket} to {agent}");
                }
                eprintln!("wteam: {e:#}");
            }
            ExitCode::FAILURE
        }
    }
}

impl Out {
    pub fn new(text: impl Into<String>, json: serde_json::Value) -> Self {
        Self {
            text: text.into(),
            json,
        }
    }
}

//! Deterministic routing: agent events on tickets move through the team graph
//! here, never through a model. The orchestrator model only decomposes,
//! dispatches and decides what to do about blocked tickets.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};

use crate::paths::Paths;
use crate::state::{EventKind, State, TicketStatus, Verdict, VerdictKind};
use crate::team::{DEFAULT_CHECK_TIMEOUT_SECS, NodeKind, Team, TeamEvent, parse_instance};
use crate::work::{self, Ticket};

/// Everything a command needs to act on one effort.
#[derive(Debug, Clone)]
pub struct Ctx {
    pub paths: Paths,
    pub team: Team,
    /// Path of the running `wteam` binary (spawned for turns).
    pub exe: PathBuf,
}

impl Ctx {
    /// Load the effort's team (state's `run.team` preset/file, `$pairs` from `run.pairs`).
    pub fn load(paths: Paths) -> Result<Ctx> {
        let state = crate::state::load(&paths)?;
        let team = crate::config::resolve_team(&paths, &state.run.team, state.run.pairs)?;
        let exe = std::env::current_exe()?;
        let ctx = Ctx { paths, team, exe };
        heal(&ctx)?;
        Ok(ctx)
    }
}

/// Whether process `pid` exists (`kill -0`).
pub fn pid_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Whether a `wteam run` is driving the orchestrator. A run flag whose watcher is dead is
/// stale: this clears it and reports the run as not live.
pub fn clear_stale_run(s: &mut State) -> bool {
    if !s.run.active {
        return false;
    }
    let live = s.run.watcher_pid.is_some_and(pid_alive);
    if !live {
        s.run.active = false;
    }
    live
}

/// Reset `busy` on agents whose turn is gone, log it, and start a turn for each agent that
/// still has mail or a held ticket. Runs on every command through [`Ctx::load`].
/// A busy agent with no pid is still spawning, unless its spawn is older than the grace period.
pub fn heal(ctx: &Ctx) -> Result<()> {
    let revived = crate::state::with_state(&ctx.paths, |s| {
        let now = crate::state::now_nanos();
        let live = clear_stale_run(s);
        let mut dead: Vec<(String, Option<u32>)> = Vec::new();
        // A live run's orchestrator turn runs in the watcher's foreground: not heal's to reset.
        let skip = |name: &str| live && parse_instance(name).0 == "orchestrator";
        for (name, a) in s
            .agents
            .iter_mut()
            .filter(|(name, a)| a.busy && !skip(name.as_str()))
        {
            let gone = match a.turn_pid {
                Some(pid) => (!pid_alive(pid)).then_some(Some(pid)),
                None => a
                    .turn_spawned_at
                    .is_none_or(|t| now.saturating_sub(t) > SPAWN_GRACE_NANOS)
                    .then_some(None),
            };
            if let Some(pid) = gone {
                a.busy = false;
                a.turn_pid = None;
                a.turn_spawned_at = None;
                dead.push((name.clone(), pid));
            }
        }
        let mut revived = Vec::new();
        for (name, pid) in dead {
            let held = s.tickets.values().any(|t| {
                t.assignee.as_deref() == Some(name.as_str())
                    && matches!(t.status, TicketStatus::Doing | TicketStatus::Review)
            });
            let offset = s.agents.get(&name).map_or(0, |a| a.mail_delivered);
            revived.push((name, pid, held, offset));
        }
        Ok(revived)
    })?;
    if revived.is_empty() {
        return Ok(());
    }
    let transport = crate::state::load(&ctx.paths)?.run.transport;
    let transport = crate::transport::for_name(&transport)?;
    for (agent, pid, held, offset) in revived {
        let log = ctx.paths.logs_dir().join(format!("{agent}-process.log"));
        std::fs::create_dir_all(ctx.paths.logs_dir())?;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log)
            .with_context(|| format!("opening {}", log.display()))?;
        let line = match pid {
            Some(pid) => format!("wteam: turn process {pid} is gone; busy reset\n"),
            None => "wteam: turn spawn never reported a pid; busy reset\n".to_string(),
        };
        std::io::Write::write_all(&mut f, line.as_bytes())?;
        let (mail, _) = crate::mailbox::read_from(&ctx.paths, &agent, offset)?;
        if held || !mail.trim().is_empty() {
            transport.deliver(ctx, &agent)?;
        }
    }
    Ok(())
}

/// Cap on diff text embedded in a review request.
const MAX_DIFF_BYTES: usize = 100_000;

/// Lines of a failed check's output returned to the worker.
const CHECK_TAIL_LINES: usize = 40;

/// How long a busy agent may lack a pid before its spawn counts as dead (30 s).
const SPAWN_GRACE_NANOS: u64 = 30_000_000_000;

/// Normalise `1`/`#01` to the two-digit ticket id and load its definition.
fn find_ticket(ctx: &Ctx, id: &str) -> Result<Ticket> {
    let id = format!("{:0>2}", id.trim().trim_start_matches('#'));
    work::load_tickets(&ctx.paths.tickets_dir())?
        .into_iter()
        .find(|t| t.id == id)
        .with_context(|| format!("no ticket {id} in {}", ctx.paths.tickets_dir().display()))
}

/// Append to `to`'s mailbox, then start or feed its turn through the run's transport.
fn mail_and_deliver(ctx: &Ctx, to: &str, from: &str, ticket: &str, body: &str) -> Result<()> {
    crate::mailbox::append_ticket(&ctx.paths, to, from, Some(ticket), body)?;
    let transport = crate::state::load(&ctx.paths)?.run.transport;
    crate::transport::for_name(&transport)?.deliver(ctx, to)
}

/// Run `git` in the repo root; stdout on success, stderr in the error otherwise.
fn git(ctx: &Ctx, args: &[&str]) -> Result<String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(&ctx.paths.root)
        .args(args)
        .output()
        .context("running git")?;
    if !out.status.success() {
        bail!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// `git diff` of the ticket's files against HEAD, new files included.
fn ticket_diff(ctx: &Ctx, t: &Ticket) -> Result<String> {
    let mut specs: Vec<&str> = vec!["--"];
    specs.extend(t.files.iter().map(String::as_str));
    let mut args = vec!["diff", "HEAD"];
    args.extend(&specs);
    let mut diff = git(ctx, &args)?;
    let mut args = vec!["ls-files", "--others", "--exclude-standard"];
    args.extend(&specs);
    for f in git(ctx, &args)?.lines() {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(&ctx.paths.root)
            .args(["diff", "--no-index", "--", "/dev/null", f])
            .output()
            .context("running git diff --no-index")?;
        diff.push_str(&String::from_utf8_lossy(&out.stdout));
    }
    if diff.len() > MAX_DIFF_BYTES {
        let mut cut = MAX_DIFF_BYTES;
        while !diff.is_char_boundary(cut) {
            cut -= 1;
        }
        diff.truncate(cut);
        diff.push_str("\n[diff truncated]\n");
    }
    Ok(diff)
}

/// Stage the ticket's files and commit only them. Returns the new sha.
fn commit_ticket(ctx: &Ctx, t: &Ticket) -> Result<String> {
    let msg = format!("feat: {} (wteam {})", t.title, t.id);
    if t.files.is_empty() {
        git(ctx, &["add", "-A"])?;
        git(ctx, &["commit", "--allow-empty", "-m", &msg])?;
    } else {
        for f in &t.files {
            // A pattern that matches nothing (a file the worker never touched) is fine.
            if let Err(e) = git(ctx, &["add", "-A", "--", f])
                && !format!("{e:#}").contains("did not match any")
            {
                return Err(e);
            }
        }
        let mut args = vec!["diff", "--cached", "--name-only", "--"];
        args.extend(t.files.iter().map(String::as_str));
        let staged = git(ctx, &args)?;
        let paths: Vec<&str> = staged.lines().collect();
        if paths.is_empty() {
            git(ctx, &["diff", "--cached", "--quiet"])
                .context("nothing in the ticket's files changed, and other changes are staged")?;
            git(ctx, &["commit", "--allow-empty", "-m", &msg])?;
        } else {
            let mut args = vec!["commit", "-m", &msg, "--"];
            args.extend(&paths);
            git(ctx, &args)?;
        }
    }
    Ok(git(ctx, &["rev-parse", "HEAD"])?.trim().to_string())
}

/// Role `orchestrator` dispatches to (first `dispatch` route), else `worker`.
fn dispatch_role(ctx: &Ctx) -> String {
    ctx.team
        .routes("orchestrator", TeamEvent::Dispatch)
        .first()
        .map_or_else(|| "worker".to_string(), |l| l.to.clone())
}

/// The worker brief: ticket, round, earlier findings and notes, and how to report back.
fn brief(
    t: &Ticket,
    round: u32,
    last: Option<&Verdict>,
    note: Option<&str>,
    contract: &str,
) -> String {
    let mut b = format!(
        "Ticket {} (round {round})\n\n{}\n{contract}",
        t.id,
        t.body.trim()
    );
    if let Some(n) = note {
        b.push_str(&format!("\nOrchestrator note:\n{}\n", n.trim()));
    }
    if let Some(v) = last {
        if !v.findings.is_empty() {
            b.push_str("\nPrevious findings to resolve:\n");
            v.findings
                .iter()
                .for_each(|f| b.push_str(&format!("- {f}\n")));
        }
        if !v.notes.is_empty() {
            b.push_str("\nReviewer notes:\n");
            v.notes.iter().for_each(|n| b.push_str(&format!("- {n}\n")));
        }
    }
    b.push_str(&format!(
        "\nWhen done run `wteam submit {id} --summary \"...\"`. If you cannot proceed run `wteam block {id} --reason \"...\"`.\n",
        id = t.id
    ));
    b
}

/// Pointer to the effort's shared contract when the planner wrote `architecture.md`.
fn contract_note(ctx: &Ctx) -> String {
    if ctx.paths.effort_dir().join("architecture.md").exists() {
        format!(
            "\nShared contract: .wteam/{}/architecture.md. Follow its signatures and rules.\n",
            ctx.paths.slug
        )
    } else {
        String::new()
    }
}

/// Append the verdict to `reviews/NN.md` and its follow-ups to `queue.md`.
fn record_review(ctx: &Ctx, t: &Ticket, round: u32, v: &Verdict) -> Result<()> {
    use std::io::Write;
    let label = match v.kind {
        VerdictKind::Pass => "PASS",
        VerdictKind::Fail => "FAIL",
    };
    let mut entry = format!("## Round {round}: {label} ({})\n", v.reviewer);
    for (title, items) in [("Findings", &v.findings), ("Notes", &v.notes)] {
        if !items.is_empty() {
            entry.push_str(&format!("\n{title}:\n"));
            items
                .iter()
                .for_each(|i| entry.push_str(&format!("- {i}\n")));
        }
    }
    if !v.follow_ups.is_empty() {
        entry.push_str("\nFollow-ups:\n");
        for (k, text) in &v.follow_ups {
            entry.push_str(&format!("- [{k}] {text}\n"));
        }
    }
    entry.push('\n');
    let dir = ctx.paths.reviews_dir();
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let file = dir.join(format!("{}.md", t.id));
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file)
        .and_then(|mut f| f.write_all(entry.as_bytes()))
        .with_context(|| format!("appending to {}", file.display()))?;
    if !v.follow_ups.is_empty() {
        let mut queue = String::new();
        for (k, text) in &v.follow_ups {
            let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
            queue.push_str(&format!("- [{k}] {}: {text} (from {})\n", t.id, v.reviewer));
        }
        let file = ctx.paths.queue_file();
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file)
            .and_then(|mut f| f.write_all(queue.as_bytes()))
            .with_context(|| format!("appending to {}", file.display()))?;
    }
    Ok(())
}

/// Where a PASS (or an approval) sends a ticket next.
enum Hop {
    /// Another reviewer takes the ticket.
    Review { role: String, index: u32 },
    /// A human gate holds the ticket until `wteam approve`.
    Human { role: String },
    /// A command gate runs before the next hop.
    Check { node: String, index: u32 },
    /// Nothing follows: commit.
    Commit,
}

/// Route a PASS from instance `role`/`index`. Any agent role except `orchestrator` is a review hop.
fn pass_hop(ctx: &Ctx, role: &str, index: u32) -> Result<Hop> {
    let Some(link) = ctx.team.routes(role, TeamEvent::Pass).into_iter().next() else {
        return Ok(Hop::Commit);
    };
    match ctx.team.roles.get(&link.to).map(|r| &r.kind) {
        Some(NodeKind::Human) => Ok(Hop::Human {
            role: link.to.clone(),
        }),
        Some(NodeKind::Agent { .. }) if link.to != "orchestrator" => {
            let index = ctx
                .team
                .targets(link, index)
                .first()
                .copied()
                .context("pass link has no target")?;
            Ok(Hop::Review {
                role: link.to.clone(),
                index,
            })
        }
        Some(NodeKind::Agent { .. }) => Ok(Hop::Commit),
        Some(NodeKind::Command { .. }) => Ok(Hop::Check {
            node: link.to.clone(),
            index,
        }),
        None => bail!("team routes PASS to unknown node `{}`", link.to),
    }
}

/// Reviewer roles on the pass chain from `start` to commit, in order. Follows the first pass link
/// at each node, through human gates and command nodes, and stops at the orchestrator or a cycle.
fn review_chain(ctx: &Ctx, start: &str) -> Vec<String> {
    let mut chain = vec![start.to_string()];
    let mut seen = vec![start.to_string()];
    let mut at = start.to_string();
    while let Some(link) = ctx.team.routes(&at, TeamEvent::Pass).into_iter().next() {
        let to = link.to.clone();
        if seen.contains(&to) {
            break;
        }
        seen.push(to.clone());
        match ctx.team.roles.get(&to).map(|r| &r.kind) {
            Some(NodeKind::Agent { .. }) if to == "orchestrator" => break,
            Some(NodeKind::Agent { .. }) => chain.push(to.clone()),
            Some(_) => {}
            None => break,
        }
        at = to;
    }
    chain
}

/// The first agent role a submit from `from` reaches, skipping command nodes.
fn first_reviewer(ctx: &Ctx, from: &str) -> Result<String> {
    let mut at = from.to_string();
    for _ in 0..=ctx.team.roles.len() {
        let link = ctx
            .team
            .routes(&at, TeamEvent::Submit)
            .into_iter()
            .next()
            .with_context(|| format!("team `{}` routes no submit from `{at}`", ctx.team.name))?;
        match ctx.team.roles.get(&link.to).map(|r| &r.kind) {
            Some(NodeKind::Agent { .. }) => return Ok(link.to.clone()),
            Some(NodeKind::Command { .. }) => at = link.to.clone(),
            _ => bail!("submit path from `{from}` reaches no reviewer"),
        }
    }
    bail!("submit path from `{from}` loops through command nodes")
}

/// Bail unless ticket `id` has status `expect` and is held by `by`.
fn ensure_holder(s: &mut State, id: &str, by: &str, expect: TicketStatus) -> Result<()> {
    let ts = s.ticket_mut(id);
    if ts.status != expect || ts.assignee.as_deref() != Some(by) {
        bail!("ticket {id} is not held by {by} for this step");
    }
    Ok(())
}

/// Kill process group `pid` (spawned with `process_group(0)`).
fn kill_group(pid: u32) {
    let _ = std::process::Command::new("kill")
        .args(["-KILL", "--", &format!("-{pid}")])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

/// Run `script` in the repo root with `WTEAM_TICKET` set, killing its process group after
/// `timeout_secs`. `None` on success, else the failure text with the output tail.
fn run_check(ctx: &Ctx, script: &str, ticket: &str, timeout_secs: u64) -> Result<Option<String>> {
    use std::io::Read;
    use std::os::unix::process::CommandExt;
    let mut child = std::process::Command::new("sh")
        .arg("-c")
        .arg(script)
        .current_dir(&ctx.paths.root)
        .env("WTEAM_TICKET", ticket)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .process_group(0)
        .spawn()
        .with_context(|| format!("running `{script}`"))?;
    let pid = child.id();
    let mut out_pipe = child.stdout.take().context("capturing check stdout")?;
    let mut err_pipe = child.stderr.take().context("capturing check stderr")?;
    let out_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = out_pipe.read_to_end(&mut buf);
        buf
    });
    let err_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = err_pipe.read_to_end(&mut buf);
        buf
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
    let (status, timed_out) = loop {
        if let Some(status) = child.try_wait()? {
            break (status, false);
        }
        if std::time::Instant::now() >= deadline {
            kill_group(pid);
            break (child.wait()?, true);
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    if status.success() && !timed_out {
        return Ok(None);
    }
    let mut text = out_reader.join().unwrap_or_default();
    text.extend(err_reader.join().unwrap_or_default());
    let text = String::from_utf8_lossy(&text);
    let lines: Vec<&str> = text.lines().collect();
    let tail = lines[lines.len().saturating_sub(CHECK_TAIL_LINES)..].join("\n");
    let head = if timed_out {
        format!("`{script}` timed out after {timeout_secs}s")
    } else {
        format!("`{script}` failed ({status})")
    };
    Ok(Some(format!("{head}:\n{tail}")))
}

/// Count a failed check as a FAIL round against `worker`, whose ticket is `Doing`.
fn gate_failed(
    ctx: &Ctx,
    t: &Ticket,
    gate: &str,
    by: &str,
    expect: TicketStatus,
    worker: &str,
    finding: String,
) -> Result<()> {
    let verdict = Verdict {
        kind: VerdictKind::Fail,
        reviewer: gate.to_string(),
        findings: vec![finding],
        notes: Vec::new(),
        follow_ups: Vec::new(),
    };
    fail_round(ctx, t, by, worker, verdict, expect)
}

/// The review request mail: ticket, contract pointer, `context`, diff and verdict instructions.
fn review_request(ctx: &Ctx, t: &Ticket, from: &str, context: &str) -> String {
    let diff = ticket_diff(ctx, t).unwrap_or_else(|e| format!("(diff unavailable: {e:#})"));
    format!(
        "Review request: ticket {id} from {from}\n\n{body}\n{contract}\n{context}\nDiff of the ticket's files against HEAD:\n~~~~diff\n{diff}\n~~~~\n\nReview against the ticket's acceptance criteria. Then run exactly one of:\n- `wteam verdict {id} pass`\n- `wteam verdict {id} fail --finding \"...\"` (repeat --finding per blocking problem)\n",
        id = t.id,
        body = t.body.trim(),
        contract = contract_note(ctx),
    )
}

/// Assign `ticket` to a worker (pair `pair`, else the first idle one), brief it, start its turn.
pub fn dispatch(ctx: &Ctx, ticket: &str, pair: Option<u32>) -> Result<String> {
    let t = find_ticket(ctx, ticket)?;
    let all = work::load_tickets(&ctx.paths.tickets_dir())?;
    let role = dispatch_role(ctx);
    let pairs = ctx.team.count(&role);
    let (agent, round, last, note) = crate::state::with_state(&ctx.paths, |s| {
        if !work::ready(&all, s).iter().any(|r| r.id == t.id) {
            bail!(
                "ticket {} is not ready (it must be todo with every blocker done)",
                t.id
            );
        }
        let mut busy_pairs = Vec::new();
        for (id, ts) in &s.tickets {
            let active = matches!(
                ts.status,
                TicketStatus::Doing | TicketStatus::Review | TicketStatus::AwaitingApproval
            );
            if !active {
                continue;
            }
            if ts.status != TicketStatus::AwaitingApproval {
                busy_pairs.extend(ts.pair);
            }
            if let Some(other) = all.iter().find(|o| &o.id == id)
                && work::files_overlap(&t, other)
            {
                bail!(
                    "ticket {} overlaps files with {id}, which is in progress",
                    t.id
                );
            }
        }
        let n = match pair {
            Some(n) if n < 1 || n > pairs => bail!("no pair {n} (team has {pairs})"),
            Some(n) if busy_pairs.contains(&n) => bail!("pair {n} is busy"),
            Some(n) => n,
            None => (1..=pairs)
                .find(|n| !busy_pairs.contains(n))
                .context("every pair is busy")?,
        };
        let agent = crate::team::instance_id(&role, n);
        let ts = s.ticket_mut(&t.id);
        ts.status = TicketStatus::Doing;
        ts.assignee = Some(agent.clone());
        ts.pair = Some(n);
        ts.blocked_reason = None;
        ts.passes.clear();
        Ok((
            agent,
            ts.rounds + 1,
            ts.last_verdict.clone(),
            ts.note.clone(),
        ))
    })?;
    mail_and_deliver(
        ctx,
        &agent,
        "orchestrator",
        &t.id,
        &brief(
            &t,
            round,
            last.as_ref(),
            note.as_deref(),
            &contract_note(ctx),
        ),
    )?;
    Ok(agent)
}

/// A worker finished a round: run the verify and command gates, then hand the ticket to the first reviewer.
pub fn submit(ctx: &Ctx, agent: &str, ticket: &str, summary: &str) -> Result<()> {
    let t = find_ticket(ctx, ticket)?;
    crate::state::with_state(&ctx.paths, |s| {
        ensure_holder(s, &t.id, agent, TicketStatus::Doing)
    })?;
    for cmd in &t.verify {
        if let Some(out) = run_check(ctx, cmd, &t.id, DEFAULT_CHECK_TIMEOUT_SECS)? {
            return gate_failed(ctx, &t, "verify", agent, TicketStatus::Doing, agent, out);
        }
    }
    let summary = if summary.trim().is_empty() {
        "(none)"
    } else {
        summary.trim()
    };
    let (mut from, index) = parse_instance(agent);
    let mut steps = 0;
    loop {
        steps += 1;
        if steps > ctx.team.roles.len() {
            bail!("submit path from `{from}` loops through command nodes");
        }
        let link = ctx
            .team
            .routes(&from, TeamEvent::Submit)
            .into_iter()
            .next()
            .with_context(|| format!("team `{}` routes no submit from `{from}`", ctx.team.name))?;
        match ctx.team.roles.get(&link.to).map(|r| &r.kind) {
            Some(NodeKind::Agent { .. }) => {
                let reviewer = crate::team::instance_id(
                    &link.to,
                    *ctx.team
                        .targets(link, index)
                        .first()
                        .context("submit link has no target")?,
                );
                crate::state::with_state(&ctx.paths, |s| {
                    let ts = s.ticket_mut(&t.id);
                    ts.status = TicketStatus::Review;
                    ts.assignee = Some(reviewer.clone());
                    Ok(())
                })?;
                let body = review_request(ctx, &t, agent, &format!("Worker summary:\n{summary}\n"));
                return mail_and_deliver(ctx, &reviewer, agent, &t.id, &body);
            }
            Some(NodeKind::Command { script, timeout }) => {
                if let Some(out) = run_check(ctx, script, &t.id, *timeout)? {
                    return gate_failed(ctx, &t, &link.to, agent, TicketStatus::Doing, agent, out);
                }
                from = link.to.clone();
            }
            Some(NodeKind::Human) => {
                bail!("a human gate cannot follow submit (ticket {})", t.id)
            }
            None => bail!("team routes submit to unknown node `{}`", link.to),
        }
    }
}

/// Record that a passed ticket moves on: to the next reviewer, to a human gate, or to commit.
fn advance_passed(
    ctx: &Ctx,
    t: &Ticket,
    by: &str,
    hop: Hop,
    notes: &[String],
    hops: usize,
) -> Result<()> {
    match hop {
        Hop::Check { node, index } => {
            if hops > ctx.team.roles.len() {
                bail!("pass path from `{by}` loops through command nodes");
            }
            let (script, timeout) = match ctx.team.roles.get(&node).map(|r| &r.kind) {
                Some(NodeKind::Command { script, timeout }) => (script.clone(), *timeout),
                _ => bail!("`{node}` is not a command node"),
            };
            if let Some(out) = run_check(ctx, &script, &t.id, timeout)? {
                let pair = crate::state::load(&ctx.paths)?
                    .tickets
                    .get(&t.id)
                    .and_then(|ts| ts.pair)
                    .unwrap_or(1);
                let worker = crate::team::instance_id(&dispatch_role(ctx), pair);
                return gate_failed(ctx, t, &node, by, TicketStatus::Review, &worker, out);
            }
            let next = pass_hop(ctx, &node, index)?;
            advance_passed(ctx, t, by, next, notes, hops + 1)
        }
        Hop::Review { role, index } => {
            let repeat = crate::state::load(&ctx.paths)?
                .tickets
                .get(&t.id)
                .is_some_and(|ts| ts.passes.contains(&role));
            if repeat {
                return block(ctx, by, &t.id, &format!("pass cycle at {role}"));
            }
            let reviewer = crate::team::instance_id(&role, index);
            crate::state::with_state(&ctx.paths, |s| {
                let ts = s.ticket_mut(&t.id);
                ts.status = TicketStatus::Review;
                ts.assignee = Some(reviewer.clone());
                Ok(())
            })?;
            let mut context = format!("`{by}` passed this round; this is the next review.\n");
            if !notes.is_empty() {
                context.push_str("Earlier reviewer notes:\n");
                notes
                    .iter()
                    .for_each(|n| context.push_str(&format!("- {n}\n")));
            }
            let body = review_request(ctx, t, by, &context);
            mail_and_deliver(ctx, &reviewer, by, &t.id, &body)
        }
        Hop::Human { role } => crate::state::with_state(&ctx.paths, |s| {
            let ts = s.ticket_mut(&t.id);
            ts.status = TicketStatus::AwaitingApproval;
            ts.assignee = Some(crate::team::instance_id(&role, 1));
            s.push_event(
                EventKind::NeedsApproval,
                Some(&t.id),
                Some(by),
                format!(
                    "ticket {} passed review; run `wteam approve {}`",
                    t.id, t.id
                ),
            );
            Ok(())
        }),
        Hop::Commit => finish_pass(ctx, t, by, notes),
    }
}

/// Commit a passed ticket once every reviewer on its pass chain has passed this round.
fn finish_pass(ctx: &Ctx, t: &Ticket, by: &str, notes: &[String]) -> Result<()> {
    let first = first_reviewer(ctx, &dispatch_role(ctx))?;
    let passes = crate::state::load(&ctx.paths)?
        .tickets
        .get(&t.id)
        .map(|ts| ts.passes.clone())
        .unwrap_or_default();
    let pending: Vec<String> = review_chain(ctx, &first)
        .into_iter()
        .filter(|r| !passes.contains(r))
        .collect();
    if !pending.is_empty() {
        bail!(
            "ticket {} cannot commit: no PASS yet from {}",
            t.id,
            pending.join(", ")
        );
    }
    let sha = commit_ticket(ctx, t)?;
    let pair = crate::state::with_state(&ctx.paths, |s| {
        let ts = s.ticket_mut(&t.id);
        ts.status = TicketStatus::Done;
        ts.commit = Some(sha.clone());
        let pair = ts.pair;
        s.push_event(
            EventKind::Passed,
            Some(&t.id),
            Some(by),
            format!("ticket {} passed review, committed {sha}", t.id),
        );
        Ok(pair)
    })?;
    if !notes.is_empty() {
        // Delivered with the worker's next brief; a finished ticket needs no turn.
        let worker = crate::team::instance_id(&dispatch_role(ctx), pair.unwrap_or(1));
        let mut body = format!("Reviewer notes on passed ticket {}:\n", t.id);
        notes
            .iter()
            .for_each(|n| body.push_str(&format!("- {n}\n")));
        crate::mailbox::append_ticket(&ctx.paths, &worker, by, Some(&t.id), &body)?;
    }
    Ok(())
}

/// A reviewer's verdict: PASS moves the ticket along its path; FAIL returns it to the worker or blocks at the cap.
pub fn verdict(ctx: &Ctx, agent: &str, ticket: &str, verdict: Verdict) -> Result<()> {
    let t = find_ticket(ctx, ticket)?;
    let (role, index) = parse_instance(agent);
    match verdict.kind {
        VerdictKind::Pass if !verdict.findings.is_empty() => {
            bail!("a PASS verdict carries no findings (use notes or follow-ups)")
        }
        VerdictKind::Fail if verdict.findings.is_empty() => {
            bail!("a FAIL verdict needs at least one --finding")
        }
        _ => {}
    }
    match verdict.kind {
        VerdictKind::Pass => {
            let hop = pass_hop(ctx, &role, index)?;
            let round = crate::state::with_state(&ctx.paths, |s| {
                ensure_holder(s, &t.id, agent, TicketStatus::Review)?;
                let ts = s.ticket_mut(&t.id);
                if !ts.passes.contains(&role) {
                    ts.passes.push(role.clone());
                }
                ts.last_verdict = Some(verdict.clone());
                Ok(ts.rounds + 1)
            })?;
            record_review(ctx, &t, round, &verdict)?;
            advance_passed(ctx, &t, agent, hop, &verdict.notes, 0)
        }
        VerdictKind::Fail => {
            let pair = crate::state::load(&ctx.paths)?
                .tickets
                .get(&t.id)
                .and_then(|ts| ts.pair)
                .unwrap_or(index);
            let worker = ctx
                .team
                .routes(&role, TeamEvent::Fail)
                .first()
                .and_then(|l| {
                    ctx.team
                        .targets(l, index)
                        .first()
                        .map(|i| crate::team::instance_id(&l.to, *i))
                })
                .unwrap_or_else(|| crate::team::instance_id("worker", pair));
            fail_round(ctx, &t, agent, &worker, verdict, TicketStatus::Review)
        }
    }
}

/// Count a FAIL round held by `by`: mail `worker` the findings, or block at the round cap.
fn fail_round(
    ctx: &Ctx,
    t: &Ticket,
    by: &str,
    worker: &str,
    verdict: Verdict,
    expect: TicketStatus,
) -> Result<()> {
    let max = ctx.team.max_visits;
    let reason = format!(
        "review failed {max} rounds: {}",
        verdict.findings.last().map_or("", String::as_str)
    );
    // Mail before the state flip: a ticket that reads `doing` with no mail looks silent.
    let rounds = crate::state::with_state(&ctx.paths, |s| {
        ensure_holder(s, &t.id, by, expect)?;
        Ok(s.ticket_mut(&t.id).rounds + 1)
    })?;
    let capped = rounds >= max;
    if capped {
        let note = format!("Ticket {} blocked: {reason}", t.id);
        crate::mailbox::append_ticket(&ctx.paths, "orchestrator.1", by, Some(&t.id), &note)?;
    } else {
        let mut body = format!(
            "Review of ticket {} failed (round {rounds}). Findings:\n",
            t.id
        );
        verdict
            .findings
            .iter()
            .for_each(|f| body.push_str(&format!("- {f}\n")));
        if !verdict.notes.is_empty() {
            body.push_str("\nReviewer notes:\n");
            verdict
                .notes
                .iter()
                .for_each(|n| body.push_str(&format!("- {n}\n")));
        }
        body.push_str(&format!(
            "\nFix them, then run `wteam submit {}` again.\n",
            t.id
        ));
        crate::mailbox::append_ticket(&ctx.paths, worker, by, Some(&t.id), &body)?;
    }
    crate::state::with_state(&ctx.paths, |s| {
        ensure_holder(s, &t.id, by, expect)?;
        let ts = s.ticket_mut(&t.id);
        ts.rounds += 1;
        ts.last_verdict = Some(verdict.clone());
        ts.passes.clear();
        if capped {
            ts.status = TicketStatus::Blocked;
            ts.blocked_reason = Some(reason.clone());
            s.push_event(EventKind::Blocked, Some(&t.id), Some(by), reason.clone());
        } else {
            ts.status = TicketStatus::Doing;
            ts.assignee = Some(worker.to_string());
        }
        Ok(())
    })?;
    record_review(ctx, t, rounds, &verdict)?;
    if capped {
        return Ok(());
    }
    let transport = crate::state::load(&ctx.paths)?.run.transport;
    crate::transport::for_name(&transport)?.deliver(ctx, worker)
}

/// Mark `ticket` blocked with `reason` and tell the orchestrator.
pub fn block(ctx: &Ctx, agent: &str, ticket: &str, reason: &str) -> Result<()> {
    let t = find_ticket(ctx, ticket)?;
    let (role, _) = parse_instance(agent);
    crate::state::with_state(&ctx.paths, |s| {
        let ts = s.ticket_mut(&t.id);
        let holder = ts.assignee.as_deref() == Some(agent);
        if !holder && role != "orchestrator" && agent != "human" {
            bail!("ticket {} is not held by {agent}", t.id);
        }
        if matches!(ts.status, TicketStatus::Done | TicketStatus::Todo) {
            bail!("ticket {} is not in progress", t.id);
        }
        ts.status = TicketStatus::Blocked;
        ts.blocked_reason = Some(reason.to_string());
        s.push_event(EventKind::Blocked, Some(&t.id), Some(agent), reason);
        Ok(())
    })?;
    let note = format!("Ticket {} blocked by {agent}: {reason}", t.id);
    crate::mailbox::append_ticket(&ctx.paths, "orchestrator.1", agent, Some(&t.id), &note)?;
    Ok(())
}

/// Pass a human gate: the ticket resumes from the gate's own pass route.
pub fn approve(ctx: &Ctx, ticket: &str) -> Result<()> {
    let t = find_ticket(ctx, ticket)?;
    let (gate, notes) = crate::state::with_state(&ctx.paths, |s| {
        let ts = s.ticket_mut(&t.id);
        if ts.status != TicketStatus::AwaitingApproval {
            let status = serde_json::to_value(ts.status)?;
            bail!(
                "ticket {} is {}, not awaiting approval",
                t.id,
                status.as_str().unwrap_or("not awaiting approval")
            );
        }
        let gate = ts
            .assignee
            .clone()
            .with_context(|| format!("ticket {} has no human gate", t.id))?;
        let notes = ts
            .last_verdict
            .as_ref()
            .map(|v| v.notes.clone())
            .unwrap_or_default();
        Ok((gate, notes))
    })?;
    let (role, index) = parse_instance(&gate);
    let hop = pass_hop(ctx, &role, index)?;
    crate::state::with_state(&ctx.paths, |s| {
        s.ticket_mut(&t.id).status = TicketStatus::Review;
        Ok(())
    })?;
    advance_passed(ctx, &t, &gate, hop, &notes, 0)
}

/// Free-form message from one instance to another: append to the mailbox, then deliver.
pub fn send(ctx: &Ctx, from: &str, to: &str, body: &str) -> Result<()> {
    let (role, index) = crate::team::parse_instance(to);
    let is_agent = matches!(
        ctx.team.roles.get(&role).map(|r| &r.kind),
        Some(crate::team::NodeKind::Agent { .. })
    );
    if !is_agent || index < 1 || index > ctx.team.count(&role) {
        bail!("no such agent `{to}` in team `{}`", ctx.team.name);
    }
    let to = &crate::team::instance_id(&role, index);
    crate::mailbox::append(&ctx.paths, to, from, body)?;
    let transport = crate::state::load(&ctx.paths)?.run.transport;
    crate::transport::for_name(&transport)?.deliver(ctx, to)
}

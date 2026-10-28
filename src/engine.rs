//! Deterministic routing: agent events on tickets move through the team graph
//! here, never through a model. The orchestrator model only decomposes,
//! dispatches and decides what to do about blocked tickets.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::paths::{Paths, WTEAM_DIR};
use crate::state::{EventKind, State, TicketStatus, Verdict, VerdictKind};
use crate::team::{DEFAULT_CHECK_TIMEOUT_SECS, NodeKind, Team, TeamEvent, parse_instance};
use crate::work::{self, Ticket};
use crate::worktree;

/// Cap on diff text embedded in a review request.
const MAX_DIFF_BYTES: usize = 100_000;
/// Lines of a failed check's output returned to the worker.
const CHECK_TAIL_LINES: usize = 40;
/// How long a busy agent may lack a pid before its spawn counts as dead (30 s).
const SPAWN_GRACE_NANOS: u64 = 30_000_000_000;
/// Consecutive failed dispatches after which a ticket blocks instead of staying ready.
pub const DISPATCH_ATTEMPTS: u32 = 3;
/// Time limit for the project worktree hook (`.wteam/hooks/worktree`).
const HOOK_TIMEOUT_SECS: u64 = 120;

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

/// Whether process `pid` exists (`kill -0` through the POSIX shell builtin).
pub fn pid_alive(pid: u32) -> bool {
    std::process::Command::new("sh")
        .args(["-c", "kill -0 \"$1\"", "sh"])
        .arg(pid.to_string())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

#[cfg(test)]
mod pid_tests {
    use super::pid_alive;

    #[test]
    fn current_process_is_alive_and_impossible_pid_is_not() {
        assert!(pid_alive(std::process::id()));
        assert!(!pid_alive(u32::MAX));
    }
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
        let mut dead: Vec<(String, String)> = Vec::new();
        // A live run's orchestrator turn runs in the watcher's foreground: not heal's to reset.
        let skip = |name: &str| live && parse_instance(name).0 == "orchestrator";
        for (name, a) in s
            .agents
            .iter_mut()
            .filter(|(name, a)| a.busy && !skip(name.as_str()))
        {
            let age = a.turn_spawned_at.map_or("no stamp".to_string(), |t| {
                format!("spawn age {}s", now.saturating_sub(t) / 1_000_000_000)
            });
            let gone = match a.turn_pid {
                Some(pid) => (!pid_alive(pid)).then(|| format!("dead pid {pid}")),
                None => a
                    .turn_spawned_at
                    .is_none_or(|t| now.saturating_sub(t) > SPAWN_GRACE_NANOS)
                    .then(|| format!("no pid, {age}")),
            };
            if let Some(reason) = gone {
                a.busy = false;
                a.turn_pid = None;
                a.turn_spawned_at = None;
                dead.push((name.clone(), reason));
            }
        }
        let mut revived = Vec::new();
        for (name, reason) in dead {
            let held = s.tickets.values().any(|t| {
                t.assignee.as_deref() == Some(name.as_str())
                    && matches!(t.status, TicketStatus::Doing | TicketStatus::Review)
            });
            let offset = s.agents.get(&name).map_or(0, |a| a.mail_delivered);
            revived.push((name, reason, held, offset));
        }
        Ok(revived)
    })?;
    if revived.is_empty() {
        return Ok(());
    }
    let transport = crate::state::load(&ctx.paths)?.run.transport;
    let transport = crate::transport::for_name(&transport)?;
    for (agent, reason, held, offset) in revived {
        crate::transport::resume::trace(
            &ctx.paths,
            &agent,
            &format!("heal reset reason={reason}; busy reset"),
        )?;
        let (mail, _) = crate::mailbox::read_from(&ctx.paths, &agent, offset)?;
        if held || !mail.trim().is_empty() {
            crate::transport::resume::trace(&ctx.paths, &agent, "heal redeliver")?;
            transport.deliver(ctx, &agent)?;
        }
    }
    Ok(())
}

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

/// Pathspec groups a ticket's diff and commit cover. `Files:` are the planner's globs
/// (`src/b/**` is documented), so that group stays glob. Extras are worker-declared, so they
/// form a literal group. A ticket without `Files:` covers everything and has no extras group.
/// Scratch and ignored paths leave every group: git refuses to add an ignored path. A group the
/// filter empties is dropped.
fn path_groups<'a>(dir: &Path, t: &'a Ticket, extras: &'a [String]) -> Vec<(bool, Vec<&'a str>)> {
    if t.files.is_empty() {
        // Whole repo minus top-level scratch/. Exclude magic needs git's normal pathspec mode.
        return vec![(false, vec![":(exclude)scratch"])];
    }
    let files: Vec<&str> = t
        .files
        .iter()
        .map(String::as_str)
        .filter(|f| is_ticket_path(dir, f))
        .collect();
    let mut groups = vec![(false, files)];
    if !extras.is_empty() {
        let kept: Vec<&str> = extras
            .iter()
            .map(String::as_str)
            .filter(|e| is_ticket_path(dir, e))
            .collect();
        groups.push((true, kept));
    }
    groups.retain(|(_, specs)| !specs.is_empty());
    groups
}

/// Top-level `scratch/` holds the worker's throwaway notes, which are never ticket work.
fn is_scratch(spec: &str) -> bool {
    spec == "scratch" || spec.starts_with("scratch/")
}

/// Whether git ignores `spec` in `dir`. Tracked paths are never ignored, so they stay committable.
fn is_ignored(dir: &Path, spec: &str) -> bool {
    std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["check-ignore", "-q", "--", spec])
        .status()
        .is_ok_and(|s| s.success())
}

/// Whether `spec` may reach `git add`: neither scratch nor ignored.
fn is_ticket_path(dir: &Path, spec: &str) -> bool {
    !is_scratch(spec) && !is_ignored(dir, spec)
}

/// Split extras into `(kept, dropped)`. Dropped extras are never ticket work.
fn split_extras(dir: &Path, extras: Vec<String>) -> (Vec<String>, Vec<String>) {
    extras.into_iter().partition(|e| is_ticket_path(dir, e))
}

/// Append `-- <specs>` when there are specs; an empty list means the whole repo.
fn push_specs<'a>(args: &mut Vec<&'a str>, specs: &[&'a str]) {
    if !specs.is_empty() {
        args.push("--");
        args.extend(specs);
    }
}

/// Run git in `dir`, literal pathspecs when `literal` is set.
fn run_git(dir: &Path, literal: bool, args: &[&str]) -> Result<String> {
    if literal {
        worktree::git_literal(dir, args)
    } else {
        worktree::git(dir, args)
    }
}

/// `git diff HEAD` for the ticket's files and extras, new files included.
fn ticket_diff(dir: &Path, t: &Ticket, extras: &[String]) -> Result<String> {
    let mut diff = String::new();
    for (literal, specs) in path_groups(dir, t, extras) {
        let mut args = vec!["--no-optional-locks", "diff", "HEAD"];
        push_specs(&mut args, &specs);
        diff.push_str(&run_git(dir, literal, &args)?);
        let mut args = vec![
            "--no-optional-locks",
            "ls-files",
            "--others",
            "--exclude-standard",
        ];
        push_specs(&mut args, &specs);
        for f in run_git(dir, literal, &args)?.lines() {
            let out = std::process::Command::new("git")
                .arg("-C")
                .arg(dir)
                .env("GIT_LITERAL_PATHSPECS", "1")
                .args([
                    "--no-optional-locks",
                    "diff",
                    "--no-index",
                    "--",
                    "/dev/null",
                    f,
                ])
                .output()
                .context("running git diff --no-index")?;
            diff.push_str(&String::from_utf8_lossy(&out.stdout));
        }
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

/// Run git in `dir` against `index` when given (`GIT_INDEX_FILE`), literal pathspecs when `literal`.
fn git_index(dir: &Path, index: Option<&Path>, literal: bool, args: &[&str]) -> Result<String> {
    let mut cmd = std::process::Command::new("git");
    cmd.arg("-C").arg(dir).args(args);
    if let Some(index) = index {
        cmd.env("GIT_INDEX_FILE", index);
    }
    if literal {
        cmd.env("GIT_LITERAL_PATHSPECS", "1");
    }
    let out = cmd.output().context("running git")?;
    if !out.status.success() {
        bail!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Unstage `scratch/` back to `base`: the worker's throwaway notes are never ticket work. Reset,
/// not rm, so a tracked scratch file keeps its entry and no deletion gets staged. A path absent from
/// `base` and the index is a no-op. `index` selects a temporary index, else the worktree's own.
fn unstage_scratch(dir: &Path, index: Option<&Path>, base: &str) -> Result<()> {
    git_index(dir, index, false, &["reset", "-q", base, "--", "scratch"]).map(drop)
}

/// Stage the ticket's files and extras and commit only them. Returns the new sha.
fn commit_ticket(dir: &Path, t: &Ticket, extras: &[String]) -> Result<String> {
    let msg = format!("feat: {} (wteam {})", t.title, t.id);
    if t.files.is_empty() {
        worktree::git(dir, &["add", "-A"])?;
        unstage_scratch(dir, None, "HEAD")?;
        worktree::git(dir, &["commit", "--allow-empty", "-m", &msg])?;
    } else {
        let mut paths: Vec<String> = Vec::new();
        for (literal, specs) in path_groups(dir, t, extras) {
            for f in specs.iter().copied() {
                // A pattern that matches nothing (a file the worker never touched) is fine.
                if let Err(e) = run_git(dir, literal, &["add", "-A", "--", f])
                    && !format!("{e:#}").contains("did not match any")
                {
                    return Err(e);
                }
            }
            // A glob in `Files:` can match scratch/ too; keep it out of this commit.
            unstage_scratch(dir, None, "HEAD")?;
            let mut args = vec!["--no-optional-locks", "diff", "--cached", "--name-only"];
            push_specs(&mut args, &specs);
            paths.extend(run_git(dir, literal, &args)?.lines().map(str::to_string));
        }
        if paths.is_empty() {
            worktree::git(dir, &["--no-optional-locks", "diff", "--cached", "--quiet"])
                .context("nothing in the ticket's files changed, and other changes are staged")?;
            worktree::git(dir, &["commit", "--allow-empty", "-m", &msg])?;
        } else {
            // Names came from git, so commit them literally: none is magic or a glob.
            let mut args = vec!["commit", "-m", &msg, "--"];
            args.extend(paths.iter().map(String::as_str));
            worktree::git_literal(dir, &args)?;
        }
    }
    Ok(worktree::git(dir, &["rev-parse", "HEAD"])?
        .trim()
        .to_string())
}

/// Commit the ticket's files and extras as one commit on the fork point `base`, dropping the
/// branch's own history (wip commits from a block). Built in a temporary index, so the worktree
/// and branch are untouched until the commit exists; only then does the worktree move to it.
fn squash_commit(
    ctx: &Ctx,
    dir: &Path,
    t: &Ticket,
    extras: &[String],
    base: &str,
) -> Result<String> {
    let index = ctx
        .paths
        .effort_dir()
        .join(format!("squash-{}.index", t.id));
    let msg = format!("feat: {} (wteam {})", t.title, t.id);
    let idx = Some(index.as_path());
    let built = (|| -> Result<String> {
        git_index(dir, idx, false, &["read-tree", base])?;
        if t.files.is_empty() {
            git_index(dir, idx, false, &["add", "-A"])?;
        } else {
            for (literal, specs) in path_groups(dir, t, extras) {
                for f in specs.iter().copied() {
                    // A pattern that matches nothing (a file the worker never touched) is fine.
                    if let Err(e) = git_index(dir, idx, literal, &["add", "-A", "--", f])
                        && !format!("{e:#}").contains("did not match any")
                    {
                        return Err(e);
                    }
                }
            }
        }
        unstage_scratch(dir, idx, base)?;
        let changed = git_index(dir, idx, false, &["diff", "--cached", "--name-only", base])?;
        if !t.files.is_empty() && changed.trim().is_empty() {
            bail!("nothing in the ticket's files changed");
        }
        let tree = git_index(dir, idx, false, &["write-tree"])?
            .trim()
            .to_string();
        let sha = worktree::git(dir, &["commit-tree", &tree, "-p", base, "-m", &msg])?
            .trim()
            .to_string();
        worktree::git(dir, &["reset", "--hard", &sha])?;
        Ok(sha)
    })();
    let _ = std::fs::remove_file(&index);
    built
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
        "\nWhen done write the summary to a file and run `wteam submit {id} --summary-file <path>`. If you cannot proceed write the reason to a file and run `wteam block {id} --reason-file <path>`.\n",
        id = t.id
    ));
    b
}

/// Pointer to the effort's shared contract when the planner wrote `architecture.md`.
fn contract_note(ctx: &Ctx) -> String {
    if ctx.paths.effort_dir().join("architecture.md").exists() {
        format!(
            "\nShared contract: {}. Follow its signatures and rules.\n",
            ctx.paths.effort_dir().join("architecture.md").display()
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

/// Run `script` in `dir` (the ticket's worktree, else the repo root) with `WTEAM_TICKET` set,
/// killing its process group after
/// `timeout_secs`. `None` on success, else the failure text with the output tail.
fn run_check(dir: &Path, script: &str, ticket: &str, timeout_secs: u64) -> Result<Option<String>> {
    use std::io::Read;
    use std::os::unix::process::CommandExt;
    let mut child = std::process::Command::new("sh")
        .arg("-c")
        .arg(script)
        .current_dir(dir)
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

/// Run the project's worktree hook (`sh .wteam/hooks/worktree <dir>`, cwd the repo root) for
/// `dir`, the pair worktree or the root. `None` when there is no hook or it passed; else the
/// failure text with the output tail.
pub fn run_hook(ctx: &Ctx, dir: &Path, ticket: &str) -> Result<Option<String>> {
    if !ctx.paths.project_hook_file("worktree").is_file() {
        return Ok(None);
    }
    let script = format!(
        "sh {WTEAM_DIR}/hooks/worktree {}",
        shell_quote(&dir.to_string_lossy())
    );
    run_check(&ctx.paths.root, &script, ticket, HOOK_TIMEOUT_SECS)
}

/// `s` as one POSIX shell word.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// The `## Project notes` section from `.wteam/brief.md`, appended verbatim. Empty when the
/// file is absent or blank.
fn project_notes(ctx: &Ctx) -> Result<String> {
    let file = ctx.paths.project_brief_file();
    if !file.is_file() {
        return Ok(String::new());
    }
    let text =
        std::fs::read_to_string(&file).with_context(|| format!("reading {}", file.display()))?;
    if text.trim().is_empty() {
        return Ok(String::new());
    }
    Ok(format!("\n## Project notes\n\n{}\n", text.trim_end()))
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

/// Where a ticket's git work happens: its pair's worktree under worktree isolation, else the
/// repo root.
fn ticket_dir(ctx: &Ctx, ticket: &str) -> Result<PathBuf> {
    let s = crate::state::load(&ctx.paths)?;
    if s.run.isolation != "worktree" {
        return Ok(ctx.paths.root.clone());
    }
    let pair = s
        .tickets
        .get(ticket)
        .and_then(|ts| ts.pair)
        .with_context(|| format!("ticket {ticket} has no pair"))?;
    Ok(worktree::path(&ctx.paths, pair))
}

/// The review request mail: ticket, contract pointer, `context`, diff and verdict instructions.
fn review_request(ctx: &Ctx, t: &Ticket, from: &str, context: &str) -> Result<String> {
    let extras = extras_of(&crate::state::load(&ctx.paths)?, &t.id);
    let diff = ticket_dir(ctx, &t.id)
        .and_then(|dir| ticket_diff(&dir, t, &extras))
        .unwrap_or_else(|e| format!("(diff unavailable: {e:#})"));
    let mut listed = String::new();
    if !extras.is_empty() {
        listed.push_str(
            "Extra paths outside Files (the worker declared these at submit; judge each one):\n",
        );
        extras
            .iter()
            .for_each(|e| listed.push_str(&format!("- {e}\n")));
        listed.push('\n');
    }
    let contract = format!("{}{}", contract_note(ctx), project_notes(ctx)?);
    Ok(format!(
        "Review request: ticket {id} from {from}\n\n{body}\n{contract}\n{context}\n{listed}Diff of the ticket's files and extras against HEAD:\n~~~~diff\n{diff}\n~~~~\n\nReview against the ticket's acceptance criteria. Then write the verdict sections (FINDINGS, NOTES, FOLLOW-UPS) to a file and run exactly one of:\n- `wteam verdict {id} pass --file <path>`\n- `wteam verdict {id} fail --file <path>` (one `- ` bullet per blocking finding)\n",
        id = t.id,
        body = t.body.trim(),
        contract = contract,
    ))
}

/// The paths a ticket's worker declared outside `Files:` at its latest submit.
fn extras_of(s: &State, id: &str) -> Vec<String> {
    s.tickets
        .get(id)
        .map(|ts| ts.extras.clone())
        .unwrap_or_default()
}

/// Assign `ticket` to a worker (pair `pair`, else the first idle one), brief it, start its turn.
pub fn dispatch(ctx: &Ctx, ticket: &str, pair: Option<u32>) -> Result<String> {
    let t = find_ticket(ctx, ticket)?;
    let all = work::load_tickets(&ctx.paths.tickets_dir())?;
    let role = dispatch_role(ctx);
    let pairs = ctx.team.count(&role);
    // Everything fallible that is not state lives before phase 1, so a failure here leaves the
    // ticket untouched.
    let notes = project_notes(ctx)?;
    // Phase 1, under the state lock: pick the pair, prepare its worktree and mark the ticket
    // `Doing` on it. `Doing` keeps the pair busy, so no other dispatch takes it while the hook
    // runs in phase 2.
    let (agent, round, last, note, dir, prior) = crate::state::with_state(&ctx.paths, |s| {
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
                && !s.run.allows_overlap()
                && work::files_overlap(&t, other)
            {
                bail!(
                    "ticket {} overlaps files with {id}, which is in progress",
                    t.id
                );
            }
        }
        let dirty = if s.run.isolation == "worktree" {
            worktree::dirty_pairs(&ctx.paths, pairs)
        } else {
            Vec::new()
        };
        // A ticket whose branch is checked out in a pair's worktree goes back to that pair.
        let holder = if s.run.isolation == "worktree" {
            worktree::holders(&ctx.paths)?.get(&t.id).copied()
        } else {
            None
        };
        let n = match (pair, holder) {
            (Some(n), _) if n < 1 || n > pairs => bail!("no pair {n} (team has {pairs})"),
            (Some(n), Some(h)) if n != h => bail!(
                "ticket {} is on the branch held by pair {h}; dispatch it to pair {h}",
                t.id
            ),
            (_, Some(h)) if busy_pairs.contains(&h) => bail!("pair {h} is busy"),
            (_, Some(h)) => h,
            (Some(n), None) if busy_pairs.contains(&n) => bail!("pair {n} is busy"),
            (Some(n), None) => n,
            (None, None) => {
                match (1..=pairs).find(|n| !busy_pairs.contains(n) && !dirty.contains(n)) {
                    Some(n) => n,
                    None if dirty.is_empty() => bail!("every pair is busy"),
                    None => {
                        let list: Vec<String> = dirty.iter().map(|n| format!("pair {n}")).collect();
                        bail!(
                            "every idle pair is busy or has uncommitted work (dirty: {})",
                            list.join(", ")
                        )
                    }
                }
            }
        };
        let dir = if s.run.isolation == "worktree" {
            Some(worktree::prepare(&ctx.paths, n, &t.id)?)
        } else {
            None
        };
        let agent = crate::team::instance_id(&role, n);
        let prior = s.ticket_mut(&t.id).clone();
        let ts = s.ticket_mut(&t.id);
        ts.status = TicketStatus::Doing;
        ts.assignee = Some(agent.clone());
        ts.pair = Some(n);
        ts.blocked_reason = None;
        ts.dispatch_failures = 0;
        ts.passes.clear();
        ts.extras.clear();
        Ok((
            agent,
            ts.rounds + 1,
            ts.last_verdict.clone(),
            ts.note.clone(),
            dir,
            prior,
        ))
    })?;
    // Phase 2 and the mail, no lock held for the hook (up to HOOK_TIMEOUT_SECS). Any error after
    // phase 1 restores the prior ticket state; the caller's dispatch-failure cap counts it.
    let sent = (|| -> Result<()> {
        if let Some(dir) = &dir {
            let failure = match run_hook(ctx, dir, &t.id) {
                Ok(None) => None,
                Ok(Some(err)) => Some(err),
                Err(e) => Some(format!("{e:#}")),
            };
            if let Some(err) = failure {
                bail!("worktree hook failed: {err}");
            }
        }
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
                &format!("{}{notes}", contract_note(ctx)),
            ),
        )
    })();
    if let Err(e) = sent {
        crate::state::with_state(&ctx.paths, |s| {
            *s.ticket_mut(&t.id) = prior;
            Ok(())
        })?;
        return Err(e);
    }
    Ok(agent)
}

/// Outcome of [`fill`]: the dispatches that stand, and the tickets whose dispatch failed.
#[derive(Debug, Default)]
pub struct Fill {
    /// `(ticket, agent)` per dispatch made.
    pub started: Vec<(String, String)>,
    /// `(ticket, error)` per dispatch that failed. The ticket stays ready unless it is in `blocked`.
    pub failed: Vec<(String, String)>,
    /// Tickets whose failed dispatch reached `DISPATCH_ATTEMPTS` and were blocked.
    pub blocked: Vec<String>,
}

/// Count a failed dispatch of `id`. At `DISPATCH_ATTEMPTS` in a row the ticket blocks with
/// `dispatch failed N times: <error>` and the orchestrator is mailed. Returns whether it blocked.
fn record_dispatch_failure(ctx: &Ctx, id: &str, error: &str) -> Result<bool> {
    let reason = crate::state::with_state(&ctx.paths, |s| {
        let ts = s.ticket_mut(id);
        ts.dispatch_failures += 1;
        if ts.dispatch_failures < DISPATCH_ATTEMPTS {
            return Ok(None);
        }
        let reason = format!("dispatch failed {DISPATCH_ATTEMPTS} times: {error}");
        ts.status = TicketStatus::Blocked;
        ts.blocked_reason = Some(reason.clone());
        s.push_event(EventKind::Blocked, Some(id), None, reason.as_str());
        Ok(Some(reason))
    })?;
    let Some(reason) = reason else {
        return Ok(false);
    };
    crate::mailbox::append_ticket(&ctx.paths, "orchestrator.1", "wteam", Some(id), &reason)?;
    Ok(true)
}

/// A `dispatch --fill` where some dispatches failed: the others still stand. The CLI exits
/// non-zero and lists `started`.
#[derive(Debug)]
pub struct FillError {
    /// `(ticket, agent)` for each dispatch that stood.
    pub started: Vec<(String, String)>,
    /// One line per failed ticket; it stays ready.
    pub message: String,
}

impl std::fmt::Display for FillError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for FillError {}

/// Dispatch every dispatchable ready ticket, lowest id first, until no ticket is left or
/// no pair is free. A ticket whose dispatch fails is recorded and skipped; the rest still run.
/// A ticket whose branch sits in a busy pair waits for that pair.
pub fn fill(ctx: &Ctx) -> Result<Fill> {
    let tickets = work::load_tickets(&ctx.paths.tickets_dir())?;
    let role = dispatch_role(ctx);
    let mut out = Fill::default();
    let mut tried = BTreeSet::new();
    loop {
        let s = crate::state::load(&ctx.paths)?;
        let worktree = s.run.isolation == "worktree";
        let dirty = if worktree {
            worktree::dirty_pairs(&ctx.paths, ctx.team.count(&role))
        } else {
            Vec::new()
        };
        let holders = if worktree {
            worktree::holders(&ctx.paths)?
        } else {
            BTreeMap::new()
        };
        let busy: BTreeSet<u32> = s
            .tickets
            .values()
            .filter(|ts| matches!(ts.status, TicketStatus::Doing | TicketStatus::Review))
            .filter_map(|ts| ts.pair)
            .collect();
        let mut candidates: Vec<String> = work::dispatchable(&tickets, &s, &dirty)
            .into_iter()
            .filter(|id| !tried.contains(id))
            .filter(|id| holders.get(id).is_none_or(|h| !busy.contains(h)))
            .collect();
        candidates.sort();
        let Some(id) = candidates.first().cloned() else {
            return Ok(out);
        };
        tried.insert(id.clone());
        match dispatch(ctx, &id, None) {
            Ok(agent) => out.started.push((id, agent)),
            Err(e) => {
                let error = format!("{e:#}");
                if record_dispatch_failure(ctx, &id, &error)? {
                    out.blocked.push(id.clone());
                }
                out.failed.push((id, error));
            }
        }
    }
}

/// Reject an `--extra` that is not one literal file path inside the repo, so a PASS
/// cannot commit outside it. The error names the extra and the rule it broke.
fn check_extra_path(extra: &str) -> Result<()> {
    let path = Path::new(extra);
    let rule = if extra.is_empty() || extra == "." {
        "must name a file, not the repo root"
    } else if extra.starts_with(':') {
        "must not start with `:` (git pathspec magic)"
    } else if path.is_absolute() {
        "must be relative to the repo root"
    } else if path.components().any(|c| c == Component::ParentDir) {
        "must not contain `..`"
    } else if extra.contains(['*', '?', '[']) {
        "must be a literal path, not a glob"
    } else if path.components().any(|c| c.as_os_str() == ".wteam") {
        "must not be under .wteam/"
    } else {
        return Ok(());
    };
    bail!("--extra `{extra}` {rule}")
}

/// Reject an extra whose ancestor directory is a symlink: git would stage through it, so
/// `link -> .wteam` would commit wteam's own state. The final component is not checked.
fn check_no_symlink(dir: &Path, extra: &str) -> Result<()> {
    let comps: Vec<Component> = Path::new(extra).components().collect();
    let mut prefix = PathBuf::new();
    for c in comps.iter().take(comps.len().saturating_sub(1)) {
        prefix.push(c);
        if let Ok(meta) = std::fs::symlink_metadata(dir.join(&prefix))
            && meta.file_type().is_symlink()
        {
            bail!(
                "--extra `{extra}` passes through symlink `{}`",
                prefix.display()
            );
        }
    }
    Ok(())
}

/// Refuse an extra that another in-progress ticket's `Files:` cover. Only under `isolation = none`,
/// where every pair shares one checkout and such a path is a real collision.
/// `prior` extras, then the new ones not already listed: a declared extra stays committed when a
/// resubmit omits it.
fn union_extras(prior: &[String], new: &[String]) -> Vec<String> {
    let mut out = prior.to_vec();
    for e in new {
        if !out.contains(e) {
            out.push(e.clone());
        }
    }
    out
}

/// Refuse a submit while any changed file in the working dir still has a conflict marker line.
/// Changed means tracked and different from HEAD, or untracked. Top-level `scratch/` is ignored.
fn check_no_markers(dir: &Path) -> Result<()> {
    let tracked = worktree::git(dir, &["--no-optional-locks", "diff", "--name-only", "HEAD"])?;
    let untracked = worktree::git(dir, &["ls-files", "--others", "--exclude-standard"])?;
    let mut stuck: Vec<String> = Vec::new();
    for f in tracked.lines().chain(untracked.lines()) {
        if f.starts_with("scratch/") || stuck.iter().any(|s| s == f) {
            continue;
        }
        let text = std::fs::read_to_string(dir.join(f)).unwrap_or_default();
        if text
            .lines()
            .any(|l| l.starts_with("<<<<<<< ") || l.starts_with(">>>>>>> "))
        {
            stuck.push(f.to_string());
        }
    }
    if !stuck.is_empty() {
        bail!("conflict markers remain in: {}", stuck.join(", "));
    }
    Ok(())
}

fn check_extras(s: &State, all: &[Ticket], t: &Ticket, extras: &[String]) -> Result<()> {
    for (id, ts) in &s.tickets {
        let active = matches!(
            ts.status,
            TicketStatus::Doing | TicketStatus::Review | TicketStatus::AwaitingApproval
        );
        if id == &t.id || !active {
            continue;
        }
        let Some(other) = all.iter().find(|o| &o.id == id) else {
            continue;
        };
        if let Some(e) = extras.iter().find(|e| work::files_cover(other, e)) {
            bail!(
                "ticket {}: extra `{e}` overlaps the files of ticket {id}, which is in progress",
                t.id
            );
        }
    }
    Ok(())
}

/// A worker finished a round: run the verify and command gates, then hand the ticket to the first reviewer.
pub fn submit(
    ctx: &Ctx,
    agent: &str,
    ticket: &str,
    summary: &str,
    extras: &[String],
) -> Result<Vec<String>> {
    let t = find_ticket(ctx, ticket)?;
    let new: Vec<String> = extras.iter().map(|e| e.trim().to_string()).collect();
    new.iter().try_for_each(|e| check_extra_path(e))?;
    let prior = extras_of(&crate::state::load(&ctx.paths)?, &t.id);
    let dir = ticket_dir(ctx, &t.id)?;
    let (extras, dropped) = split_extras(&dir, union_extras(&prior, &new));
    extras.iter().try_for_each(|e| check_no_symlink(&dir, e))?;
    check_no_markers(&dir)?;
    let all = work::load_tickets(&ctx.paths.tickets_dir())?;
    crate::state::with_state(&ctx.paths, |s| {
        ensure_holder(s, &t.id, agent, TicketStatus::Doing)?;
        if s.run.isolation != "worktree" {
            check_extras(s, &all, &t, &extras)?;
        }
        s.ticket_mut(&t.id).extras = extras.clone();
        Ok(())
    })?;
    route_submission(ctx, &t, agent, summary)?;
    Ok(dropped)
}

/// Run the verify gates, then route the submitted round to the first reviewer or command node.
fn route_submission(ctx: &Ctx, t: &Ticket, agent: &str, summary: &str) -> Result<()> {
    let dir = ticket_dir(ctx, &t.id)?;
    for cmd in &t.verify {
        if let Some(out) = run_check(&dir, cmd, &t.id, DEFAULT_CHECK_TIMEOUT_SECS)? {
            return gate_failed(ctx, t, "verify", agent, TicketStatus::Doing, agent, out);
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
                let body = review_request(ctx, t, agent, &format!("Worker summary:\n{summary}\n"))?;
                return mail_and_deliver(ctx, &reviewer, agent, &t.id, &body);
            }
            Some(NodeKind::Command { script, timeout }) => {
                if let Some(out) = run_check(&dir, script, &t.id, *timeout)? {
                    return gate_failed(ctx, t, &link.to, agent, TicketStatus::Doing, agent, out);
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
            if let Some(out) = run_check(&ticket_dir(ctx, &t.id)?, &script, &t.id, timeout)? {
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
            let body = review_request(ctx, t, by, &context)?;
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

/// Commit the ticket, then under worktree isolation merge it onto main. Returns the landed sha,
/// or the conflicting paths when main moved under the ticket.
/// Under worktree isolation the commit comes first: a failed commit leaves the worktree as the
/// worker left it. A branch with history past its fork point (wip commits) is squashed into the
/// one commit. Afterwards the worktree is cleaned of everything uncommitted before the merge.
fn land(ctx: &Ctx, t: &Ticket) -> Result<worktree::MergeOutcome> {
    let st = crate::state::load(&ctx.paths)?;
    let dir = ticket_dir(ctx, &t.id)?;
    let extras = extras_of(&st, &t.id);
    if st.run.isolation != "worktree" {
        return Ok(worktree::MergeOutcome::Merged(commit_ticket(
            &dir, t, &extras,
        )?));
    }
    let base = worktree::fork_point(&ctx.paths, &t.id)?;
    let head = worktree::git(&dir, &["rev-parse", "HEAD"])?;
    if head.trim() == base {
        commit_ticket(&dir, t, &extras)?;
    } else {
        squash_commit(ctx, &dir, t, &extras, &base)?;
    }
    worktree::git(&dir, &["reset", "--hard"])?;
    worktree::git(&dir, &["clean", "-fdq"])?;
    let pair = st
        .tickets
        .get(&t.id)
        .and_then(|ts| ts.pair)
        .context("worktree ticket has no pair")?;
    worktree::merge_back(&ctx.paths, pair, &t.id)
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
    let sha = match land(ctx, t) {
        Ok(worktree::MergeOutcome::Merged(sha)) => sha,
        Ok(worktree::MergeOutcome::Conflict(paths)) => {
            return landing_conflict(ctx, t, by, &paths);
        }
        Err(e) => return block(ctx, by, &t.id, &format!("{e:#}")),
    };
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

/// A passed ticket whose landing conflicted with main goes back to its pair's worker. wteam
/// stages the ticket's work on current main as uncommitted edits with conflict markers, and the
/// worker resolves them. Blocks at the visit cap.
fn landing_conflict(ctx: &Ctx, t: &Ticket, by: &str, paths: &[String]) -> Result<()> {
    let listed = paths.join(", ");
    let max = ctx.team.max_visits;
    let pair = crate::state::load(&ctx.paths)?
        .tickets
        .get(&t.id)
        .and_then(|ts| ts.pair)
        .with_context(|| format!("ticket {} has no pair", t.id))?;
    let conflicts = crate::state::with_state(&ctx.paths, |s| {
        ensure_holder(s, &t.id, by, TicketStatus::Review)?;
        let ts = s.ticket_mut(&t.id);
        ts.land_conflicts += 1;
        Ok(ts.land_conflicts)
    })?;
    if conflicts >= max {
        let reason = format!("landing conflicted with main {conflicts} times in: {listed}");
        return block(ctx, by, &t.id, &reason);
    }
    if let Err(e) = worktree::restage_on_main(&ctx.paths, pair, &t.id) {
        return block(ctx, by, &t.id, &format!("{e:#}"));
    }
    let worker = crate::team::instance_id(&dispatch_role(ctx), pair);
    // Mail before the state flip, as in `fail_round`: the worker reads the mail on its turn.
    let body = format!(
        "Landing ticket {} conflicted with main in: {listed}. wteam moved your work onto current \
         main; those files now contain conflict markers. Resolve every marker keeping both sides' \
         intent, re-run your checks, then submit again.\n",
        t.id
    );
    crate::mailbox::append_ticket(&ctx.paths, &worker, by, Some(&t.id), &body)?;
    crate::state::with_state(&ctx.paths, |s| {
        ensure_holder(s, &t.id, by, TicketStatus::Review)?;
        let ts = s.ticket_mut(&t.id);
        ts.status = TicketStatus::Doing;
        ts.assignee = Some(worker.clone());
        ts.passes.clear();
        Ok(())
    })?;
    let transport = crate::state::load(&ctx.paths)?.run.transport;
    crate::transport::for_name(&transport)?.deliver(ctx, &worker)
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
    let kept = if capped {
        keep_pair_work(ctx, &t.id)?
    } else {
        None
    };
    if capped {
        let mut note = format!("Ticket {} blocked: {reason}", t.id);
        if let Some(kept) = kept {
            note.push('\n');
            note.push_str(&kept);
        }
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

/// Whether `agent` may block `ticket` now: it holds the ticket, or it is the orchestrator or a
/// human, and the ticket is in progress.
fn check_blockable(s: &State, ticket: &str, agent: &str, role: &str) -> Result<()> {
    let ts = s.tickets.get(ticket);
    let holder = ts.and_then(|ts| ts.assignee.as_deref()) == Some(agent);
    if !holder && role != "orchestrator" && agent != "human" {
        bail!("ticket {ticket} is not held by {agent}");
    }
    if matches!(
        ts.map(|ts| ts.status),
        None | Some(TicketStatus::Done | TicketStatus::Todo)
    ) {
        bail!("ticket {ticket} is not in progress");
    }
    Ok(())
}

/// Every path that blocks a ticket calls this first. Under worktree isolation it commits the
/// ticket's pair's uncommitted work as a wip commit on the ticket's branch, so the pair is clean
/// and reusable. Returns the note to report when that fails and the pair stays dirty.
pub fn keep_pair_work(ctx: &Ctx, ticket: &str) -> Result<Option<String>> {
    let s = crate::state::load(&ctx.paths)?;
    if s.run.isolation != "worktree" {
        return Ok(None);
    }
    let Some(pair) = s.tickets.get(ticket).and_then(|ts| ts.pair) else {
        return Ok(None);
    };
    let t = find_ticket(ctx, ticket)?;
    Ok(
        match worktree::preserve(&ctx.paths, pair, &t.id, &t.title) {
            Ok(_) => None,
            Err(e) => Some(format!(
                "Uncommitted work left in pair {pair}, not preserved: {e:#}"
            )),
        },
    )
}

/// Block `id`, held by `agent`, with `reason`. The pair's work is kept first, outside the state
/// lock; the flip re-checks that `agent` still holds the ticket. Returns the kept-work note on
/// failure, as [`keep_pair_work`] does.
pub fn block_held(ctx: &Ctx, agent: &str, id: &str, reason: &str) -> Result<Option<String>> {
    let kept = keep_pair_work(ctx, id)?;
    crate::state::with_state(&ctx.paths, |s| {
        let expect = s.tickets.get(id).map(|ts| ts.status).unwrap_or_default();
        ensure_holder(s, id, agent, expect)?;
        let ts = s.ticket_mut(id);
        ts.status = TicketStatus::Blocked;
        ts.blocked_reason = Some(reason.to_string());
        s.push_event(EventKind::Blocked, Some(id), Some(agent), reason);
        Ok(())
    })?;
    Ok(kept)
}

/// Mark `ticket` blocked with `reason` and tell the orchestrator. Under worktree isolation the
/// pair's uncommitted work is first committed to the ticket's branch as a wip commit, so the
/// pair is reusable at once. A failed commit leaves the pair dirty and the orchestrator is told.
pub fn block(ctx: &Ctx, agent: &str, ticket: &str, reason: &str) -> Result<()> {
    let t = find_ticket(ctx, ticket)?;
    let (role, _) = parse_instance(agent);
    let s = crate::state::load(&ctx.paths)?;
    check_blockable(&s, &t.id, agent, &role)?;
    let kept = keep_pair_work(ctx, &t.id)?;
    crate::state::with_state(&ctx.paths, |s| {
        check_blockable(s, &t.id, agent, &role)?;
        let ts = s.ticket_mut(&t.id);
        ts.status = TicketStatus::Blocked;
        ts.blocked_reason = Some(reason.to_string());
        s.push_event(EventKind::Blocked, Some(&t.id), Some(agent), reason);
        Ok(())
    })?;
    let mut note = format!("Ticket {} blocked by {agent}: {reason}", t.id);
    if let Some(kept) = kept {
        note.push('\n');
        note.push_str(&kept);
    }
    crate::mailbox::append_ticket(&ctx.paths, "orchestrator.1", agent, Some(&t.id), &note)?;
    Ok(())
}

/// `requeue --fresh`: discard blocked `ticket`'s preserved work (its worktree and branch). Refuses
/// when another active ticket holds the same pair or the ticket's worker is still in a turn.
/// Clears the agent sessions bound to the ticket. A refusal leaves git and state unchanged.
/// Runs under the state lock and takes the merge lock inside it, the only nesting order used.
pub fn discard_preserved(paths: &Paths, s: &mut State, ticket: &str) -> Result<()> {
    let Some(ts) = s.tickets.get(ticket) else {
        return Ok(());
    };
    let (own_pair, assignee) = (ts.pair, ts.assignee.clone());
    let holder = if s.run.isolation == "worktree" {
        worktree::holders(paths)?.get(ticket).copied()
    } else {
        None
    };
    for pair in [own_pair, holder].into_iter().flatten() {
        let other = s.tickets.iter().find(|(id, o)| {
            id.as_str() != ticket
                && o.pair == Some(pair)
                && matches!(
                    o.status,
                    TicketStatus::Doing | TicketStatus::Review | TicketStatus::AwaitingApproval
                )
        });
        if let Some((other, _)) = other {
            bail!("pair {pair} is busy with {other}");
        }
    }
    if let Some(agent) = assignee.as_deref()
        && s.agents.get(agent).is_some_and(|a| a.busy)
    {
        bail!("{agent} is still running a turn for {ticket}");
    }
    if s.run.isolation == "worktree" {
        worktree::discard(paths, ticket)?;
    }
    // A worker or reviewer resumes the session of the ticket it last worked on; that conversation
    // belongs to the discarded work, so the next turn starts fresh with the role prompt.
    for agent in s
        .agents
        .values_mut()
        .filter(|a| a.session_ticket.as_deref() == Some(ticket))
    {
        agent.session_id = None;
        agent.session_turns = 0;
        agent.session_ticket = None;
    }
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

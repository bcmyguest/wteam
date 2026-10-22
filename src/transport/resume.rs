//! The default transport: each turn is a headless CLI call that resumes the
//! agent's stored session. `deliver` spawns a detached `wteam __turn <agent>`
//! process; [`run_turns`] is that process's body.

use std::io::Write;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};

use super::Transport;
use crate::adapters::{self, CommandSpec, TurnOutput, TurnRequest};
use crate::engine::Ctx;
use crate::state::{self, State, TicketStatus};
use crate::team::{NodeKind, agents, parse_instance};

pub struct Resume;

impl Transport for Resume {
    fn name(&self) -> &'static str {
        "resume"
    }

    fn deliver(&self, ctx: &Ctx, agent: &str) -> Result<()> {
        // The watcher runs the orchestrator's turns in the foreground. Its mail waits in the
        // mailbox for the next digest instead of starting a concurrent detached turn.
        if parse_instance(agent).0 == "orchestrator" {
            let live = state::with_state(&ctx.paths, |s| Ok(crate::engine::clear_stale_run(s)))?;
            if live {
                return Ok(());
            }
        }
        let spawned = claim(&ctx.paths, agent)?;
        let Some(stamp) = spawned else {
            trace(&ctx.paths, agent, "deliver claim refused: busy")?;
            return Ok(());
        };
        trace(&ctx.paths, agent, &format!("deliver claim stamp={stamp}"))?;
        match spawn_turn(ctx, agent) {
            Ok(pid) => {
                trace(&ctx.paths, agent, &format!("deliver spawned pid={pid}"))?;
                // One lock for decide and write: a stop between them would leave a running
                // child recorded on an idle agent.
                let decision =
                    state::with_state(&ctx.paths, |s| Ok(record_or_kill(s, agent, stamp, pid)))?;
                trace(
                    &ctx.paths,
                    agent,
                    &format!("deliver record pid={pid} decision={decision:?}"),
                )?;
                if decision == Decision::KillChild {
                    if crate::engine::pid_alive(pid) {
                        crate::commands::stop::kill_group(pid);
                        log_line(
                            ctx,
                            agent,
                            "wteam: child released before pid record; terminated\n",
                        )?;
                    } else {
                        log_line(ctx, agent, "wteam: child already exited\n")?;
                    }
                }
                Ok(())
            }
            Err(e) => {
                trace(&ctx.paths, agent, &format!("deliver spawn error: {e:#}"))?;
                state::with_state(&ctx.paths, |s| {
                    let st = s.agents.entry(agent.to_string()).or_default();
                    st.busy = false;
                    st.turn_pid = None;
                    st.turn_spawned_at = None;
                    Ok(())
                })?;
                Err(e)
            }
        }
    }
}

/// Outcome of the post-spawn write for the child just started.
#[derive(Debug, PartialEq, Eq)]
enum Decision {
    /// The spawn is still current: its pid is now recorded on the agent.
    Recorded,
    /// Released (stop, heal or a fast finish) or superseded: the child must be terminated.
    KillChild,
}

/// The body of `deliver`'s post-spawn lock. Records `pid` only while `agent` is busy with
/// this spawn (matching stamp); otherwise it touches nothing and says the child must die.
fn record_or_kill(s: &mut State, agent: &str, stamp: u64, pid: u32) -> Decision {
    let st = s.agents.entry(agent.to_string()).or_default();
    if st.busy && st.turn_spawned_at == Some(stamp) {
        st.turn_pid = Some(pid);
        Decision::Recorded
    } else {
        Decision::KillChild
    }
}

/// Append one line to the agent's process log.
fn log_line(ctx: &Ctx, agent: &str, line: &str) -> Result<()> {
    trace(&ctx.paths, agent, line.trim_end())
}

/// Append `msg` to `agent`'s process log, prefixed with the current UTC time
/// (`2026-10-22T20:01:02Z deliver claim stamp=…`). The log is the turn-lifecycle trace.
pub fn trace(paths: &crate::paths::Paths, agent: &str, msg: &str) -> Result<()> {
    std::fs::create_dir_all(paths.logs_dir())?;
    let path = paths.logs_dir().join(format!("{agent}-process.log"));
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut f| writeln!(f, "{} {msg}", utc_iso(secs)))
        .with_context(|| format!("writing {}", path.display()))
}

/// `secs` since the Unix epoch as UTC ISO-8601 (`YYYY-MM-DDTHH:MM:SSZ`).
fn utc_iso(secs: i64) -> String {
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Mark `agent` busy for a new turn. Busy, the old pid and the spawn stamp change under one
/// lock, so a concurrent heal never sees busy with a previous turn's pid. `None` if already busy.
fn claim(paths: &crate::paths::Paths, agent: &str) -> Result<Option<u64>> {
    state::with_state(paths, |s| {
        let (role, index) = parse_instance(agent);
        let st = s.agents.entry(agent.to_string()).or_default();
        if st.role.is_empty() {
            st.role = role;
            st.index = index;
        }
        if st.busy {
            return Ok(None);
        }
        let stamp = state::now_nanos();
        st.busy = true;
        st.turn_pid = None;
        st.turn_spawned_at = Some(stamp);
        Ok(Some(stamp))
    })
}

/// Mark `agent` busy for a turn run by the watcher in the foreground. The adapter child's
/// pid is recorded once it spawns (see `run_spec`), never the watcher's own. `false` if the
/// agent is already busy.
pub fn begin_foreground(paths: &crate::paths::Paths, agent: &str) -> Result<bool> {
    Ok(claim(paths, agent)?.is_some())
}

/// Mark a foreground turn over: `agent` is idle again.
pub fn end_foreground(paths: &crate::paths::Paths, agent: &str) -> Result<()> {
    state::with_state(paths, |s| {
        release(s, agent);
        Ok(())
    })
}

/// Start `wteam __turn <agent>` in its own process group, output to a process log.
/// Returns the child pid (also its process group id).
fn spawn_turn(ctx: &Ctx, agent: &str) -> Result<u32> {
    std::fs::create_dir_all(ctx.paths.logs_dir())?;
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(ctx.paths.logs_dir().join(format!("{agent}-process.log")))
        .context("opening process log")?;
    let child = Command::new(&ctx.exe)
        .arg("__turn")
        .arg(agent)
        .env("WTEAM_ROOT", &ctx.paths.root)
        .env("WTEAM_SLUG", &ctx.paths.slug)
        .env("WTEAM_AGENT", agent)
        .current_dir(&ctx.paths.root)
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .process_group(0)
        .spawn()
        .with_context(|| format!("spawning {} __turn {agent}", ctx.exe.display()))?;
    Ok(child.id())
}

/// Mark `agent` not busy and forget its turn process. Every path to idle goes through here.
pub fn release(s: &mut State, agent: &str) {
    let st = s.agents.entry(agent.to_string()).or_default();
    st.busy = false;
    st.turn_pid = None;
    st.turn_spawned_at = None;
}

/// Point `st`'s session at `ticket`. A session that works on another ticket is dropped, so the
/// next turn starts fresh with the role prompt. The same ticket keeps its session (a FAIL fix,
/// a landing-conflict return or a requeue of the same ticket resumes it).
pub fn bind_session(st: &mut crate::state::AgentState, ticket: &str) {
    if st.session_ticket.as_deref() != Some(ticket) {
        st.session_id = None;
        st.session_turns = 0;
        st.session_ticket = Some(ticket.to_string());
    }
}

/// What the loop does when an agent's mailbox has no undelivered mail.
enum Settle {
    /// Nothing held: the agent goes idle.
    Idle,
    /// Held ticket and this is the first silent turn: mail this re-prompt and keep going.
    Reprompt(String),
    /// Held ticket after a silent turn: block it (ticket id, reason). The caller blocks it outside
    /// the state lock and tells the orchestrator.
    Blocked(String, String),
}

/// Decide the fate of a silent turn (no mail left, ticket still held). Mutates `s` only.
fn settle(s: &mut State, agent: &str, role: &str) -> Settle {
    let held = s.tickets.iter().find_map(|(id, t)| {
        (t.assignee.as_deref() == Some(agent)
            && matches!(t.status, TicketStatus::Doing | TicketStatus::Review))
        .then(|| (id.clone(), t.status))
    });
    let held = held.filter(|_| role != "orchestrator");
    let Some((id, status)) = held else {
        s.agents.entry(agent.to_string()).or_default().silent_turns = 0;
        return Settle::Idle;
    };
    let st = s.agents.entry(agent.to_string()).or_default();
    if st.silent_turns == 0 {
        st.silent_turns = 1;
        let verb = if status == TicketStatus::Review {
            format!("`wteam verdict {id} pass|fail`")
        } else {
            format!(
                "`wteam submit {id} --summary-file <path>` or `wteam block {id} --reason-file <path>`"
            )
        };
        return Settle::Reprompt(format!("You ended your turn without {verb}. Do one now."));
    }
    st.silent_turns = 0;
    let reason = if status == TicketStatus::Review {
        format!("{role} ended without a verdict")
    } else {
        format!("{role} ended without submitting")
    };
    Settle::Blocked(id, reason)
}

/// Run turns for `agent` until its mailbox has no undelivered mail, then settle the silence.
pub fn run_turns(ctx: &Ctx, agent: &str) -> Result<()> {
    let mut ran = false;
    loop {
        let next = state::with_state(&ctx.paths, |s| {
            let (role, index) = parse_instance(agent);
            let st = s.agents.entry(agent.to_string()).or_default();
            if st.role.is_empty() {
                st.role = role.clone();
                st.index = index;
            }
            let (text, end) = crate::mailbox::read_from(&ctx.paths, agent, st.mail_delivered)?;
            if !text.trim().is_empty() {
                if !matches!(role.as_str(), "orchestrator" | "planner") {
                    let latest = crate::mailbox::parse(&text)
                        .into_iter()
                        .rev()
                        .find_map(|m| m.ticket);
                    if let Some(ticket) = latest {
                        bind_session(st, &ticket);
                    }
                }
                let first = st.session_id.is_none() && st.session_turns == 0;
                st.mail_delivered = end;
                let turn = st.turns + 1;
                let session = st.session_id.clone();
                return Ok(Turn::Run(first, session, turn, prompt_of(&text)));
            }
            Ok(match settle(s, agent, &role) {
                Settle::Reprompt(body) => Turn::Reprompt(body),
                Settle::Idle => {
                    release(s, agent);
                    Turn::Stop
                }
                Settle::Blocked(id, reason) => Turn::Blocked(id, reason),
            })
        });
        let next = match next {
            Ok(n) => n,
            Err(e) => {
                // Mail offset may have advanced: never leave the agent marked busy.
                state::with_state(&ctx.paths, |s| {
                    release(s, agent);
                    Ok(())
                })?;
                return Err(e);
            }
        };
        match next {
            Turn::Stop => {
                trace(&ctx.paths, agent, "release idle")?;
                return Ok(());
            }
            Turn::Reprompt(body) => {
                crate::mailbox::append(&ctx.paths, agent, "wteam", &body)?;
            }
            Turn::Blocked(id, reason) => {
                // The agent stays busy while its pair's work is committed, then is released.
                let blocked = crate::engine::block_held(ctx, agent, &id, &reason);
                state::with_state(&ctx.paths, |s| {
                    release(s, agent);
                    Ok(())
                })?;
                trace(&ctx.paths, agent, &format!("release blocked {id}"))?;
                let mut note = format!("Ticket {id} blocked by {agent}: {reason}");
                if let Some(kept) = blocked? {
                    note.push('\n');
                    note.push_str(&kept);
                }
                crate::mailbox::append(&ctx.paths, "orchestrator.1", agent, &note)?;
                return Ok(());
            }
            Turn::Run(first, session, turn, prompt) => {
                if ran {
                    trace(&ctx.paths, agent, "mail at turn end: next turn")?;
                }
                ran = true;
                trace(
                    &ctx.paths,
                    agent,
                    &format!("turn start turn={turn} first={first}"),
                )?;
                if let Err(e) = one_turn(ctx, agent, first, session, turn, &prompt, false) {
                    trace(&ctx.paths, agent, &format!("turn end turn={turn} error"))?;
                    fail_turn(ctx, agent, &format!("{e:#}"))?;
                    return Ok(());
                }
                trace(&ctx.paths, agent, &format!("turn end turn={turn}"))?;
            }
        }
    }
}

/// One step of the turn loop, decided under the state lock.
enum Turn {
    Run(bool, Option<String>, u32, String),
    Reprompt(String),
    Blocked(String, String),
    Stop,
}

/// The turn prompt: the undelivered messages, each headed by its sender.
fn prompt_of(text: &str) -> String {
    crate::mailbox::parse(text)
        .iter()
        .map(|m| {
            let t = m
                .ticket
                .as_deref()
                .map(|t| format!(" (ticket {t})"))
                .unwrap_or_default();
            format!("Message from {}{t}:\n{}", m.from, m.body)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Write the files a translated profile needs, backing up the originals first.
fn apply_files(paths: &crate::paths::Paths, t: &crate::permissions::Translation) -> Result<()> {
    for (rel, contents) in &t.files {
        crate::backup::write_tracked(paths, rel, contents)?;
    }
    Ok(())
}

/// Working directory for `agent`'s turn: the worktree of the pair holding one of its tickets
/// under worktree isolation, else the repo root.
fn turn_dir(ctx: &Ctx, agent: &str) -> Result<PathBuf> {
    let s = crate::state::load(&ctx.paths)?;
    if s.run.isolation == "worktree" {
        let held = s.tickets.values().find(|ts| {
            ts.assignee.as_deref() == Some(agent)
                && matches!(
                    ts.status,
                    crate::state::TicketStatus::Doing
                        | crate::state::TicketStatus::Review
                        | crate::state::TicketStatus::AwaitingApproval
                )
        });
        if let Some(pair) = held.and_then(|ts| ts.pair) {
            let dir = crate::worktree::path(&ctx.paths, pair);
            if dir.is_dir() {
                return Ok(dir);
            }
        }
    }
    Ok(ctx.paths.root.clone())
}

/// Run one adapter turn for `agent`, write its log and record the outcome.
/// Returns the reply text. `foreground` turns (the watcher's orchestrator turns) run the
/// adapter in its own process group and record its pid as the agent's `turn_pid`, so
/// `wteam stop` ends the adapter without touching the watcher.
pub fn one_turn(
    ctx: &Ctx,
    agent: &str,
    first: bool,
    session: Option<String>,
    turn: u32,
    prompt: &str,
    foreground: bool,
) -> Result<String> {
    let (role, _) = parse_instance(agent);
    let team = state::load(&ctx.paths)?.run.team;
    let dirs = crate::config::search_dirs(&ctx.paths, &team);
    let search = dirs.agents.clone();
    let agent_name = match ctx.team.roles.get(&role).map(|r| &r.kind) {
        Some(NodeKind::Agent { agent, .. }) => agent.clone(),
        _ => bail!("`{role}` is not an agent role in team `{}`", ctx.team.name),
    };
    let def = agents::load(&agent_name, &search)?;
    let adapter = adapters::for_cli(&def.cli)?;
    let cwd = turn_dir(ctx, agent)?;
    let permissions = if def.cli == "fake" {
        Default::default()
    } else {
        let profile =
            crate::permissions::load(def.profile.as_deref().unwrap_or("edit"), &dirs.profiles)?;
        let t = crate::permissions::translate_in(&def.cli, &profile, &role, &cwd)?;
        apply_files(&ctx.paths, &t)?;
        t
    };
    let role_prompt = if first {
        Some(agents::role_prompt(
            def.prompt.as_deref().unwrap_or(&role),
            &search,
        )?)
    } else {
        None
    };
    let req = TurnRequest {
        agent_id: agent.to_string(),
        model: def.model.clone(),
        session_id: session.clone(),
        prompt: prompt.to_string(),
        role_prompt,
        cwd: cwd.clone(),
        env: vec![
            ("WTEAM_ROOT".into(), ctx.paths.root.display().to_string()),
            ("WTEAM_SLUG".into(), ctx.paths.slug.clone()),
            ("WTEAM_AGENT".into(), agent.to_string()),
        ],
        extra_args: def.extra_args.clone(),
        permissions,
    };
    let spec = adapter.command(&req);
    let log = ctx.paths.logs_dir().join(format!("{agent}-{turn}.log"));
    let track = foreground.then_some((&ctx.paths, agent));
    let (stdout, stderr, ok) = run_with_retries(&spec, track)?;
    std::fs::create_dir_all(ctx.paths.logs_dir())?;
    std::fs::write(&log, format!("{stdout}{stderr}"))
        .with_context(|| format!("writing {}", log.display()))?;
    if !ok {
        bail!(
            "{} exited non-zero (log: {}): {}",
            spec.program,
            log.display(),
            tail(&format!("{stderr}{stdout}"))
        );
    }
    let out: TurnOutput = adapter
        .parse_output(&stdout)
        .with_context(|| format!("parsing {} output (log: {})", def.cli, log.display()))?;
    let session_id = out
        .session_id
        .or(session)
        .or_else(|| (def.cli == "fake").then(|| format!("fake-{agent}")));
    let tokens = out.context_tokens.or_else(|| {
        session_id
            .as_deref()
            .and_then(|id| adapter.context_tokens_fallback(id))
    });
    state::with_state(&ctx.paths, |s| {
        let st = s.agents.entry(agent.to_string()).or_default();
        st.session_id = session_id;
        st.context_tokens = tokens.or(st.context_tokens);
        st.tokens_total += tokens.unwrap_or(0);
        st.turns += 1;
        st.session_turns += 1;
        Ok(())
    })?;
    Ok(out.text)
}

/// Backoff before each retry of a transient adapter failure; three retries, then the turn fails.
const BACKOFF_SECS: [u64; 3] = [5, 20, 60];

/// Output fragments that mark an adapter failure as transient (rate limit, overload, network).
const TRANSIENT: [&str; 5] = ["429", "rate limit", "overloaded", "econnreset", "timed out"];

/// Whether `text` (adapter stdout+stderr) names a transient failure.
fn is_transient(text: &str) -> bool {
    let lower = text.to_lowercase();
    TRANSIENT.iter().any(|p| lower.contains(p))
}

/// `WTEAM_BACKOFF_SCALE` multiplies every backoff delay; tests set 0.
fn backoff_scale() -> f64 {
    std::env::var("WTEAM_BACKOFF_SCALE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1.0)
}

/// Run `spec`, retrying transient failures with backoff. Returns the last attempt.
///
/// A retry re-runs the whole adapter command. Side effects of a partly run attempt (files
/// written, `wteam` calls already made) are not undone and repeat on the next attempt.
fn run_with_retries(
    spec: &CommandSpec,
    track: Option<(&crate::paths::Paths, &str)>,
) -> Result<(String, String, bool)> {
    let mut attempt = 0;
    loop {
        let (stdout, stderr, ok) = run_spec(spec, track)?;
        let transient = !ok && is_transient(&format!("{stdout}{stderr}"));
        match BACKOFF_SECS.get(attempt) {
            Some(secs) if transient => {
                let delay = *secs as f64 * backoff_scale();
                std::thread::sleep(std::time::Duration::from_secs_f64(delay));
                attempt += 1;
            }
            _ => return Ok((stdout, stderr, ok)),
        }
    }
}

/// Last lines of `text`, for error messages.
fn tail(text: &str) -> String {
    let lines: Vec<&str> = text.trim().lines().collect();
    lines[lines.len().saturating_sub(10)..].join("\n")
}

/// Run `spec` to completion: (stdout, stderr, exit success). With `track`, the child gets its
/// own process group and its pid becomes that agent's `turn_pid` while it runs.
fn run_spec(
    spec: &CommandSpec,
    track: Option<(&crate::paths::Paths, &str)>,
) -> Result<(String, String, bool)> {
    let mut cmd = Command::new(&spec.program);
    cmd.args(&spec.args)
        .envs(spec.env.iter().cloned())
        .current_dir(&spec.cwd)
        .stdin(if spec.stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if track.is_some() {
        cmd.process_group(0);
    }
    let mut child = cmd
        .spawn()
        .with_context(|| format!("running `{}`", spec.program))?;
    if let Some((paths, agent)) = track {
        let pid = child.id();
        state::with_state(paths, |s| {
            s.agents.entry(agent.to_string()).or_default().turn_pid = Some(pid);
            Ok(())
        })?;
    }
    let writer = spec
        .stdin
        .clone()
        .zip(child.stdin.take())
        .map(|(input, mut pipe)| {
            std::thread::spawn(move || {
                let _ = pipe.write_all(input.as_bytes());
            })
        });
    let out = child.wait_with_output()?;
    // The child is gone: clear busy and its pid in the same write, so heal never sees a dead
    // tracked pid on a busy agent.
    if let Some((paths, agent)) = track {
        state::with_state(paths, |s| {
            release(s, agent);
            Ok(())
        })?;
    }
    if let Some(w) = writer {
        let _ = w.join();
    }
    Ok((
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.success(),
    ))
}

/// A turn failed: block the agent's ticket, tell the orchestrator, go idle.
fn fail_turn(ctx: &Ctx, agent: &str, error: &str) -> Result<()> {
    let reason = format!("{agent} turn failed: {error}");
    let held: Vec<String> = state::with_state(&ctx.paths, |s| {
        Ok(s.tickets
            .iter()
            .filter(|(_, t)| {
                t.assignee.as_deref() == Some(agent)
                    && matches!(t.status, TicketStatus::Doing | TicketStatus::Review)
            })
            .map(|(id, _)| id.clone())
            .collect())
    })?;
    // The agent stays busy while each pair's work is committed, then is released.
    let mut body = reason.clone();
    let blocked = held.iter().try_for_each(|id| -> Result<()> {
        if let Some(kept) = crate::engine::block_held(ctx, agent, id, &reason)? {
            body.push_str(&format!("\n{id}: {kept}"));
        }
        Ok(())
    });
    state::with_state(&ctx.paths, |s| {
        release(s, agent);
        Ok(())
    })?;
    blocked?;
    let ticket = held.first().map(String::as_str);
    crate::mailbox::append_ticket(&ctx.paths, "orchestrator.1", agent, ticket, &body)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::Paths;

    #[test]
    fn apply_files_writes_codex_rules_and_records_them() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path(), "s");
        let profile = crate::permissions::builtin("edit").unwrap();
        let t = crate::permissions::translate("codex", &profile, "worker").unwrap();
        assert!(!t.files.is_empty());
        apply_files(&paths, &t).unwrap();
        for (rel, contents) in &t.files {
            assert_eq!(
                &std::fs::read_to_string(tmp.path().join(rel)).unwrap(),
                contents
            );
        }
        assert!(crate::backup::pending(&paths));
        let restored = crate::backup::restore(&paths).unwrap();
        assert_eq!(restored.len(), t.files.len());
        assert!(!tmp.path().join(&t.files[0].0).exists());
    }

    #[test]
    fn stop_between_claim_and_pid_write_kills_the_child() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path(), "s");
        let stamp = claim(&paths, "worker.1").unwrap().unwrap();
        // The post-spawn write, normal case: recorded.
        let d =
            state::with_state(&paths, |s| Ok(record_or_kill(s, "worker.1", stamp, 4242))).unwrap();
        assert_eq!(d, Decision::Recorded);
        assert_eq!(
            state::load(&paths).unwrap().agents["worker.1"].turn_pid,
            Some(4242)
        );
        // `wteam stop` releases the turn (which also clears the pid), then the write runs.
        state::with_state(&paths, |s| {
            release(s, "worker.1");
            Ok(())
        })
        .unwrap();
        let d =
            state::with_state(&paths, |s| Ok(record_or_kill(s, "worker.1", stamp, 5555))).unwrap();
        assert_eq!(d, Decision::KillChild);
        let st = &state::load(&paths).unwrap().agents["worker.1"];
        assert!(!st.busy && st.turn_pid.is_none(), "{st:?}");
        // A newer claim does not make the old spawn current.
        let newer = claim(&paths, "worker.1").unwrap().unwrap();
        let d =
            state::with_state(&paths, |s| Ok(record_or_kill(s, "worker.1", stamp, 6666))).unwrap();
        assert_eq!(d, Decision::KillChild);
        let d =
            state::with_state(&paths, |s| Ok(record_or_kill(s, "worker.1", newer, 7777))).unwrap();
        assert_eq!(d, Decision::Recorded);
    }

    #[test]
    fn bind_session_drops_only_another_tickets_session() {
        let mut st = crate::state::AgentState {
            session_id: Some("S1".into()),
            session_turns: 2,
            session_ticket: Some("01".into()),
            ..Default::default()
        };
        bind_session(&mut st, "01");
        assert_eq!(st.session_id.as_deref(), Some("S1"));
        assert_eq!(st.session_turns, 2);
        bind_session(&mut st, "02");
        assert_eq!(st.session_id, None);
        assert_eq!(st.session_turns, 0);
        assert_eq!(st.session_ticket.as_deref(), Some("02"));
    }

    #[test]
    fn utc_iso_formats_epoch_and_a_known_instant() {
        assert_eq!(utc_iso(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc_iso(1_700_000_000), "2023-11-14T22:13:20Z");
        assert_eq!(utc_iso(951_782_400), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn claim_clears_a_stale_pid() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path(), "s");
        state::with_state(&paths, |s| {
            let st = s.agents.entry("worker.1".into()).or_default();
            st.turn_pid = Some(4_000_000);
            Ok(())
        })
        .unwrap();
        assert!(claim(&paths, "worker.1").unwrap().is_some());
        let st = &state::load(&paths).unwrap().agents["worker.1"];
        assert!(st.busy);
        assert_eq!(st.turn_pid, None);
        assert!(st.turn_spawned_at.is_some());
        // A second claim while busy is refused.
        assert!(claim(&paths, "worker.1").unwrap().is_none());
    }
}

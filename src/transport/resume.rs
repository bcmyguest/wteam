//! The default transport: each turn is a headless CLI call that resumes the
//! agent's stored session. `deliver` spawns a detached `wteam __turn <agent>`
//! process; [`run_turns`] is that process's body.

use std::io::Write;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

use super::Transport;
use crate::adapters::{self, CommandSpec, TurnOutput, TurnRequest};
use crate::engine::Ctx;
use crate::state::{self, EventKind, State, TicketStatus};
use crate::team::{NodeKind, agents, parse_instance};

pub struct Resume;

impl Transport for Resume {
    fn name(&self) -> &'static str {
        "resume"
    }

    fn deliver(&self, ctx: &Ctx, agent: &str) -> Result<()> {
        let spawned = claim(&ctx.paths, agent)?;
        let Some(stamp) = spawned else {
            return Ok(());
        };
        match spawn_turn(ctx, agent) {
            Ok(pid) => {
                // One lock for decide and write: a stop between them would leave a running
                // child recorded on an idle agent.
                let decision =
                    state::with_state(&ctx.paths, |s| Ok(record_or_kill(s, agent, stamp, pid)))?;
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
    use std::io::Write;
    std::fs::create_dir_all(ctx.paths.logs_dir())?;
    let path = ctx.paths.logs_dir().join(format!("{agent}-process.log"));
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut f| f.write_all(line.as_bytes()))
        .with_context(|| format!("writing {}", path.display()))
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

/// What the loop does when an agent's mailbox has no undelivered mail.
enum Settle {
    /// Nothing held: the agent goes idle.
    Idle,
    /// Held ticket and this is the first silent turn: mail this re-prompt and keep going.
    Reprompt(String),
    /// Held ticket after a silent turn: the ticket is blocked, the orchestrator told.
    Blocked(String),
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
            format!("`wteam submit {id}` or `wteam block {id}`")
        };
        return Settle::Reprompt(format!("You ended your turn without {verb}. Do one now."));
    }
    st.silent_turns = 0;
    let reason = if status == TicketStatus::Review {
        format!("{role} ended without a verdict")
    } else {
        format!("{role} ended without submitting")
    };
    let t = s.ticket_mut(&id);
    t.status = TicketStatus::Blocked;
    t.blocked_reason = Some(reason.clone());
    s.push_event(EventKind::Blocked, Some(&id), Some(agent), reason.clone());
    Settle::Blocked(format!("Ticket {id} blocked by {agent}: {reason}"))
}

/// Run turns for `agent` until its mailbox has no undelivered mail, then settle the silence.
pub fn run_turns(ctx: &Ctx, agent: &str) -> Result<()> {
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
                let first = st.session_id.is_none() && st.turns == 0;
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
                Settle::Blocked(note) => {
                    release(s, agent);
                    Turn::Blocked(note)
                }
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
            Turn::Stop => return Ok(()),
            Turn::Reprompt(body) => {
                crate::mailbox::append(&ctx.paths, agent, "wteam", &body)?;
            }
            Turn::Blocked(note) => {
                crate::mailbox::append(&ctx.paths, "orchestrator.1", agent, &note)?;
                return Ok(());
            }
            Turn::Run(first, session, turn, prompt) => {
                if let Err(e) = one_turn(ctx, agent, first, session, turn, &prompt) {
                    fail_turn(ctx, agent, &format!("{e:#}"))?;
                    return Ok(());
                }
            }
        }
    }
}

/// One step of the turn loop, decided under the state lock.
enum Turn {
    Run(bool, Option<String>, u32, String),
    Reprompt(String),
    Blocked(String),
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

/// Agent definition search path: project dir, then the user's config dir.
fn search_dirs(ctx: &Ctx) -> Vec<PathBuf> {
    vec![
        ctx.paths.project_agents_dir(),
        crate::config::config_dir().join("agents"),
    ]
}

/// Write the files a translated profile needs, backing up the originals first.
fn apply_files(paths: &crate::paths::Paths, t: &crate::permissions::Translation) -> Result<()> {
    for (rel, contents) in &t.files {
        crate::backup::write_tracked(paths, rel, contents)?;
    }
    Ok(())
}

/// Run one adapter turn for `agent`, write its log and record the outcome.
fn one_turn(
    ctx: &Ctx,
    agent: &str,
    first: bool,
    session: Option<String>,
    turn: u32,
    prompt: &str,
) -> Result<()> {
    let (role, _) = parse_instance(agent);
    let search = search_dirs(ctx);
    let agent_name = match ctx.team.roles.get(&role).map(|r| &r.kind) {
        Some(NodeKind::Agent { agent, .. }) => agent.clone(),
        _ => bail!("`{role}` is not an agent role in team `{}`", ctx.team.name),
    };
    let def = agents::load(&agent_name, &search)?;
    let adapter = adapters::for_cli(&def.cli)?;
    let permissions = if def.cli == "fake" {
        Default::default()
    } else {
        let profiles = [
            ctx.paths.project_profiles_dir(),
            crate::config::config_dir().join("profiles"),
        ];
        let profile =
            crate::permissions::load(def.profile.as_deref().unwrap_or("edit"), &profiles)?;
        let t = crate::permissions::translate(&def.cli, &profile, &role)?;
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
        cwd: ctx.paths.root.clone(),
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
    let (stdout, stderr, ok) = run_with_retries(&spec)?;
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
        Ok(())
    })
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
fn run_with_retries(spec: &CommandSpec) -> Result<(String, String, bool)> {
    let mut attempt = 0;
    loop {
        let (stdout, stderr, ok) = run_spec(spec)?;
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

/// Run `spec` to completion: (stdout, stderr, exit success).
fn run_spec(spec: &CommandSpec) -> Result<(String, String, bool)> {
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
    let mut child = cmd
        .spawn()
        .with_context(|| format!("running `{}`", spec.program))?;
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
    let held = state::with_state(&ctx.paths, |s| {
        release(s, agent);
        let held: Vec<String> = s
            .tickets
            .iter()
            .filter(|(_, t)| {
                t.assignee.as_deref() == Some(agent)
                    && matches!(t.status, TicketStatus::Doing | TicketStatus::Review)
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in &held {
            let t = s.ticket_mut(id);
            t.status = TicketStatus::Blocked;
            t.blocked_reason = Some(reason.clone());
            s.push_event(EventKind::Blocked, Some(id), Some(agent), reason.clone());
        }
        Ok(held)
    })?;
    let ticket = held.first().map(String::as_str);
    crate::mailbox::append_ticket(&ctx.paths, "orchestrator.1", agent, ticket, &reason)?;
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

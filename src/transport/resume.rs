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
use crate::state::{self, EventKind, TicketStatus};
use crate::team::{NodeKind, agents, parse_instance};

pub struct Resume;

impl Transport for Resume {
    fn name(&self) -> &'static str {
        "resume"
    }

    fn deliver(&self, ctx: &Ctx, agent: &str) -> Result<()> {
        let start = state::with_state(&ctx.paths, |s| {
            let (role, index) = parse_instance(agent);
            let st = s.agents.entry(agent.to_string()).or_default();
            if st.role.is_empty() {
                st.role = role;
                st.index = index;
            }
            Ok(!std::mem::replace(&mut st.busy, true))
        })?;
        if !start {
            return Ok(());
        }
        if let Err(e) = spawn_turn(ctx, agent) {
            state::with_state(&ctx.paths, |s| {
                s.agents.entry(agent.to_string()).or_default().busy = false;
                Ok(())
            })?;
            return Err(e);
        }
        Ok(())
    }
}

/// Start `wteam __turn <agent>` in its own process group, output to a process log.
fn spawn_turn(ctx: &Ctx, agent: &str) -> Result<()> {
    std::fs::create_dir_all(ctx.paths.logs_dir())?;
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(ctx.paths.logs_dir().join(format!("{agent}-process.log")))
        .context("opening process log")?;
    Command::new(&ctx.exe)
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
    Ok(())
}

/// Run turns for `agent` until its mailbox has no undelivered mail, then mark it idle.
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
            if text.trim().is_empty() {
                st.busy = false;
                st.turn_pid = None;
                let stalled = s.tickets.iter().find_map(|(id, t)| {
                    (t.assignee.as_deref() == Some(agent) && t.status == TicketStatus::Doing)
                        .then(|| id.clone())
                });
                if let (true, Some(id)) = (role != "orchestrator", stalled) {
                    s.push_event(
                        EventKind::Idle,
                        Some(&id),
                        Some(agent),
                        format!("{agent} ended its turn without submitting ticket {id}"),
                    );
                }
                return Ok(None);
            }
            st.turn_pid = Some(std::process::id());
            let first = st.session_id.is_none() && st.turns == 0;
            st.mail_delivered = end;
            Ok(Some((
                first,
                st.session_id.clone(),
                st.turns + 1,
                prompt_of(&text),
            )))
        })?;
        let Some((first, session, turn, prompt)) = next else {
            return Ok(());
        };
        if let Err(e) = one_turn(ctx, agent, first, session, turn, &prompt) {
            fail_turn(ctx, agent, &format!("{e:#}"))?;
            return Ok(());
        }
    }
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
    let (stdout, stderr, ok) = run_spec(&spec)?;
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
        st.turns += 1;
        Ok(())
    })
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
        let st = s.agents.entry(agent.to_string()).or_default();
        st.busy = false;
        st.turn_pid = None;
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
}

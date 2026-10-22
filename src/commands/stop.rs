//! `wteam stop` and `wteam restore`.

use anyhow::Result;

use super::Out;
use crate::cli::Global;
use crate::paths::Paths;
use crate::state;

/// Restore tracked files; the text lists what came back.
fn restore_files(paths: &Paths) -> Result<(Vec<String>, String)> {
    let restored = crate::backup::restore(paths)?;
    let text = if restored.is_empty() {
        "nothing to restore".to_string()
    } else {
        format!(
            "restored {} file(s):\n{}",
            restored.len(),
            restored.join("\n")
        )
    };
    Ok((restored, text))
}

/// SIGTERM the process group led by `pid`; a group that is already gone is fine.
pub fn kill_group(pid: u32) {
    let _ = std::process::Command::new("kill")
        .args(["-TERM", "--", &format!("-{pid}")])
        .stderr(std::process::Stdio::null())
        .status();
}

/// Kill the turn process group of every busy agent and mark them stopped.
fn stop_agents(paths: &Paths) -> Result<Vec<String>> {
    let stopped = state::with_state(paths, |s| {
        let busy: Vec<(String, Option<u32>)> = s
            .agents
            .iter()
            .filter(|(_, a)| a.busy)
            .map(|(id, a)| (id.clone(), a.turn_pid))
            .collect();
        let mut stopped = Vec::new();
        for (id, pid) in busy {
            if let Some(pid) = pid {
                kill_group(pid);
            }
            crate::transport::resume::release(s, &id);
            stopped.push(id);
        }
        // The run is over: the watcher's orchestrator turn ends with the adapter it killed.
        s.run.active = false;
        Ok(stopped)
    })?;
    Ok(stopped)
}

pub fn stop(g: &Global) -> Result<Out> {
    let paths = super::effort(g)?;
    let stopped = stop_agents(&paths)?;
    let (restored, restore_text) = restore_files(&paths)?;
    // A report failure must not undo or hide the cleanup above.
    let report = crate::report::stop_report(&paths);
    let mut text = format!("stopped {} agent(s)\n{restore_text}", stopped.len());
    match &report {
        Ok(r) => text.push_str(&format!("\n\n{r}")),
        Err(e) => text.push_str(&format!("\n\nstop report unavailable: {e:#}")),
    }
    Ok(Out::new(
        text,
        serde_json::json!({
            "stopped": stopped,
            "restored": restored,
            "report": report.ok(),
        }),
    ))
}

pub fn restore(g: &Global) -> Result<Out> {
    let paths = super::effort(g)?;
    let (restored, text) = restore_files(&paths)?;
    Ok(Out::new(text, serde_json::json!({ "restored": restored })))
}

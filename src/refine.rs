//! Post-run trajectory analysis: tokens, stop reasons, unlanded work, grade gaps.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde_json::{Value, json};

use crate::paths::Paths;
use crate::report;
use crate::state::{self, TicketStatus};

#[derive(Debug, Clone, serde::Serialize)]
pub struct Recommendation {
    pub id: String,
    pub priority: u8,
    pub category: String,
    pub summary: String,
    pub action: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TrajectoryReport {
    pub stop_reason: Option<String>,
    pub grade_failures: Vec<String>,
    pub tokens_by_agent: HashMap<String, u64>,
    pub dirty_worktrees: Vec<u32>,
    pub worktree_ahead: Vec<Value>,
    pub recommendations: Vec<Recommendation>,
}

/// Analyze a finished or stopped effort. Optional bench logs and grade output live outside the repo.
pub fn analyze(
    paths: &Paths,
    extra_logs: Option<&Path>,
    grade_file: Option<&Path>,
    run_log: Option<&Path>,
) -> Result<TrajectoryReport> {
    let state = state::load(paths)?;
    let stop_reason = run_log.and_then(parse_run_log);
    let mut tokens_by_agent = HashMap::new();
    for (id, a) in &state.agents {
        tokens_by_agent.insert(id.clone(), a.tokens_total);
    }
    let default_logs = paths.logs_dir();
    let log_dir = extra_logs.unwrap_or(default_logs.as_path());
    ingest_cursor_tokens(log_dir, &mut tokens_by_agent)?;
    let grade_failures = grade_file
        .map(parse_grade_failures)
        .transpose()?
        .unwrap_or_default();
    let worktree_ahead = scan_worktrees_ahead(paths)?;
    let report_json = report::report_data(paths)?;
    let dirty_worktrees: Vec<u32> = report_json["dirty_worktrees"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_u64().map(|n| n as u32))
                .collect()
        })
        .unwrap_or_default();
    let recommendations = build_recommendations(
        paths,
        &state,
        stop_reason.as_deref(),
        &dirty_worktrees,
        &worktree_ahead,
        &grade_failures,
        &report_json,
        extra_logs.is_some(),
    );
    Ok(TrajectoryReport {
        stop_reason,
        grade_failures,
        tokens_by_agent,
        dirty_worktrees,
        worktree_ahead,
        recommendations,
    })
}

fn parse_run_log(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    text.lines()
        .find(|l| l.starts_with("Run stopped:"))
        .and_then(|l| {
            l.strip_prefix("Run stopped:")
                .map(|r| r.trim().trim_end_matches('.').to_string())
        })
}

fn ingest_cursor_tokens(dir: &Path, totals: &mut HashMap<String, u64>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if !stem.contains('.') {
            continue;
        }
        let role_key = stem.split('.').next().unwrap_or(stem).to_string();
        let mut add = 0u64;
        for line in fs::read_to_string(&path)?.lines() {
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            if v.get("type").and_then(|t| t.as_str()) != Some("result") {
                continue;
            }
            let Some(usage) = v.get("usage").and_then(|u| u.as_object()) else {
                continue;
            };
            for key in [
                "inputTokens",
                "cacheReadTokens",
                "cacheWriteTokens",
                "outputTokens",
            ] {
                if let Some(n) = usage.get(key).and_then(|x| x.as_u64()) {
                    add += n;
                }
            }
        }
        if add > 0 {
            *totals.entry(role_key).or_insert(0) += add;
        }
    }
    Ok(())
}

fn parse_grade_failures(path: &Path) -> Result<Vec<String>> {
    let text = fs::read_to_string(path).with_context(|| path.display().to_string())?;
    let mut out = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("test ") else {
            continue;
        };
        if let Some(name) = rest.strip_suffix(" ... FAILED") {
            out.push(name.to_string());
        }
    }
    Ok(out)
}

fn scan_worktrees_ahead(paths: &Paths) -> Result<Vec<Value>> {
    let wt_root = paths.worktrees_dir();
    if !wt_root.is_dir() {
        return Ok(Vec::new());
    }
    let main = git_rev(&paths.root, "HEAD").unwrap_or_default();
    let mut rows = Vec::new();
    for entry in fs::read_dir(&wt_root)? {
        let entry = entry?;
        let wt = entry.path();
        if !wt.is_dir() {
            continue;
        }
        let head = git_rev(&wt, "HEAD").unwrap_or_default();
        if head.is_empty() || head == main {
            continue;
        }
        let subject = git_oneline(&wt, &head);
        rows.push(json!({
            "path": wt.strip_prefix(&paths.root).unwrap_or(&wt).display().to_string(),
            "head": head,
            "subject": subject,
        }));
    }
    Ok(rows)
}

fn git_rev(cwd: &Path, rev: &str) -> Result<String> {
    let out = std::process::Command::new("git")
        .args(["-C", &cwd.display().to_string(), "rev-parse", rev])
        .output()?;
    if !out.status.success() {
        return Ok(String::new());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn git_oneline(cwd: &Path, rev: &str) -> String {
    std::process::Command::new("git")
        .args([
            "-C",
            &cwd.display().to_string(),
            "log",
            "-1",
            "--format=%s",
            rev,
        ])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
fn build_recommendations(
    paths: &Paths,
    st: &state::State,
    stop_reason: Option<&str>,
    dirty: &[u32],
    worktree_ahead: &[Value],
    grade_failures: &[String],
    report_json: &Value,
    bench_logs: bool,
) -> Vec<Recommendation> {
    let mut recs = Vec::new();
    let exe_note = std::env::current_exe()
        .ok()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "wteam".into());

    if source_tree_lacks_cursor(paths) {
        recs.push(Recommendation {
            id: "stale-tree-wteam".into(),
            priority: 1,
            category: "harness".into(),
            summary: "Checkout source still advertises cursor as unimplemented".into(),
            action: format!(
                "Run the harness with a current wteam binary on PATH (e.g. export PATH=\"$(dirname {exe_note}):$PATH\"); rebuild with `cargo build --release` in the wteam repo before bench runs."
            ),
        });
    }

    for (id, ts) in &st.tickets {
        if ts.status != TicketStatus::Blocked {
            continue;
        }
        let reason = ts.blocked_reason.as_deref().unwrap_or("");
        if reason.contains("cursor adapter is not implemented") {
            recs.push(Recommendation {
                id: format!("blocked-{id}-cursor-stale"),
                priority: 1,
                category: "harness".into(),
                summary: format!("Ticket {id} blocked on a stale cursor error while reviewer uses cli=cursor"),
                action: format!(
                    "Rebuild/install wteam with cursor support, put `{exe_note}` first on PATH, then `wteam requeue {id} --note \"retry with current wteam on PATH\"` and `wteam run`."
                ),
            });
        }
    }

    if !worktree_ahead.is_empty() {
        recs.push(Recommendation {
            id: "unlanded-worktrees".into(),
            priority: 1,
            category: "process".into(),
            summary: format!(
                "{} worktree(s) have commits not on main — grade uses the main tree",
                worktree_ahead.len()
            ),
            action: "Finish review/submit/merge for those tickets before grading; requeue tickets stuck in todo with notes pointing at the worktree branch.".into(),
        });
    }

    if !dirty.is_empty() {
        recs.push(Recommendation {
            id: "dirty-worktrees".into(),
            priority: 2,
            category: "process".into(),
            summary: format!("Dirty worktrees: pair {}", dirty.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(", pair ")),
            action: "Resolve WIP in worktrees (submit, block with reason, or clean) so auto dispatch can use idle pairs.".into(),
        });
    }

    if let Some(reason) = stop_reason {
        if reason.contains("ready tickets were not dispatched") {
            recs.push(Recommendation {
                id: "ready-not-dispatched".into(),
                priority: 2,
                category: "orchestrator".into(),
                summary: "Run stopped with dispatchable tickets idle".into(),
                action: "Use `wteam dispatch --fill` each orchestrator turn; fix blocked/dirty pairs first.".into(),
            });
        }
        if reason.contains("no ticket is runnable") {
            recs.push(Recommendation {
                id: "nothing-runnable".into(),
                priority: 2,
                category: "orchestrator".into(),
                summary: "Run stopped with no runnable work".into(),
                action:
                    "Requeue blocked tickets after fixing infra; widen plan or reduce blockers."
                        .into(),
            });
        }
    }

    for fail in grade_failures {
        if fail.starts_with("fill_") {
            recs.push(Recommendation {
                id: format!("grade-{fail}"),
                priority: 1,
                category: "implementation".into(),
                summary: format!("Hidden grade failed: {fail}"),
                action: "Implement `wteam dispatch --fill` per engine::fill + tests/parallel.rs on main (not only in a worktree).".into(),
            });
            break;
        }
    }
    if grade_failures
        .iter()
        .any(|f| f.contains("orchestrator_runs_one_command"))
    {
        recs.push(Recommendation {
            id: "grade-orchestrator-role".into(),
            priority: 2,
            category: "implementation".into(),
            summary: "Orchestrator role text missing fill / one-command rules".into(),
            action: "Update roles/orchestrator.md with one shell command per turn and `wteam dispatch --fill` guidance.".into(),
        });
    }
    if grade_failures
        .iter()
        .any(|f| f.contains("one_dispatch_per_turn") || f.contains("kickoff_and_digest"))
    {
        recs.push(Recommendation {
            id: "grade-watcher-liveness".into(),
            priority: 2,
            category: "implementation".into(),
            summary: "Watcher liveness or fill hints failed".into(),
            action: "Port watcher idle-turn reset and FILL_HINT kickoff/digest from main wteam."
                .into(),
        });
    }

    if let Some(blocked) = report_json.get("blocked").and_then(|b| b.as_array()) {
        for b in blocked {
            let id = b.get("id").and_then(|x| x.as_str()).unwrap_or("");
            let action = b.get("action").and_then(|x| x.as_str()).unwrap_or("");
            if action.contains("CLI log") {
                recs.push(Recommendation {
                    id: format!("blocked-log-{id}"),
                    priority: 3,
                    category: "debug".into(),
                    summary: format!("Ticket {id} blocked on agent turn failure"),
                    action: action.to_string(),
                });
            }
        }
    }

    if bench_logs {
        recs.push(Recommendation {
            id: "bench-external-logs".into(),
            priority: 4,
            category: "measurement".into(),
            summary: "External bench logs supplied — token totals include planner/agent jsonl".into(),
            action: "Compare worker vs orchestrator token keys; cap replans and prefer dispatch --fill over per-ticket chains.".into(),
        });
    }

    recs.sort_by(|a, b| a.priority.cmp(&b.priority).then_with(|| a.id.cmp(&b.id)));
    recs
}

fn source_tree_lacks_cursor(paths: &Paths) -> bool {
    let mod_rs = paths.root.join("src/adapters/mod.rs");
    let Ok(text) = fs::read_to_string(mod_rs) else {
        return false;
    };
    text.contains("cursor adapter is not implemented")
}

/// Apply low-risk fixes that do not edit product source (requeue with notes only).
pub fn apply(paths: &Paths, report: &TrajectoryReport) -> Result<Vec<String>> {
    let mut done = Vec::new();
    for rec in &report.recommendations {
        if rec.id.starts_with("blocked-") && rec.id.ends_with("-cursor-stale") {
            let id = rec
                .id
                .strip_prefix("blocked-")
                .and_then(|s| s.strip_suffix("-cursor-stale"))
                .unwrap_or("");
            if id.is_empty() {
                continue;
            }
            let note = "refine: retry after putting current wteam (cursor adapter) on PATH";
            state::with_state(paths, |s| {
                let ts = s.ticket_mut(id);
                if ts.status != TicketStatus::Blocked {
                    return Ok(());
                }
                ts.status = TicketStatus::Todo;
                ts.rounds = 0;
                ts.dispatch_failures = 0;
                ts.assignee = None;
                ts.pair = None;
                ts.blocked_reason = None;
                ts.note = Some(note.to_string());
                Ok(())
            })?;
            done.push(format!("requeued ticket {id}"));
        }
    }
    Ok(done)
}

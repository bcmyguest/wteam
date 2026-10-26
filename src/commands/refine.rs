//! `wteam refine` — post-run trajectory and recommendations.

use std::path::PathBuf;

use anyhow::Result;
use clap::Args;
use serde_json::json;

use super::Out;
use crate::cli::Global;

#[derive(Debug, Args)]
pub struct RefineArgs {
    /// Bench or external log directory (plan.jsonl, run.txt, …).
    #[arg(long)]
    pub logs: Option<PathBuf>,
    /// Hidden grade output to extract failed tests.
    #[arg(long)]
    pub grade: Option<PathBuf>,
    /// Run log containing `Run stopped: …` (default: `<logs>/run.txt`).
    #[arg(long)]
    pub run_log: Option<PathBuf>,
    /// Requeue tickets when refine recommends safe infra fixes.
    #[arg(long)]
    pub apply: bool,
}

pub fn run(g: &Global, a: &RefineArgs) -> Result<Out> {
    let paths = super::effort(g)?;
    let run_log = a
        .run_log
        .clone()
        .or_else(|| a.logs.as_ref().map(|d| d.join("run.txt")));
    let report = crate::refine::analyze(
        &paths,
        a.logs.as_deref(),
        a.grade.as_deref(),
        run_log.as_deref(),
    )?;
    let applied = if a.apply {
        crate::refine::apply(&paths, &report)?
    } else {
        Vec::new()
    };
    let text = format_report(&report, &applied);
    let json = json!({
        "stop_reason": report.stop_reason,
        "grade_failures": report.grade_failures,
        "tokens_by_agent": report.tokens_by_agent,
        "dirty_worktrees": report.dirty_worktrees,
        "worktree_ahead": report.worktree_ahead,
        "recommendations": report.recommendations,
        "applied": applied,
    });
    Ok(Out::new(text, json))
}

fn format_report(report: &crate::refine::TrajectoryReport, applied: &[String]) -> String {
    let mut out = String::from("wteam refine — trajectory\n");
    if let Some(r) = &report.stop_reason {
        out.push_str(&format!("\nStop: {r}\n"));
    }
    if !report.grade_failures.is_empty() {
        out.push_str("\nGrade failures:\n");
        for f in &report.grade_failures {
            out.push_str(&format!("- {f}\n"));
        }
    }
    if !report.worktree_ahead.is_empty() {
        out.push_str("\nWorktrees ahead of main:\n");
        for w in &report.worktree_ahead {
            out.push_str(&format!(
                "- {} {} — {}\n",
                w["path"].as_str().unwrap_or("?"),
                w["head"]
                    .as_str()
                    .map(|h| &h[..h.len().min(7)])
                    .unwrap_or(""),
                w["subject"].as_str().unwrap_or("")
            ));
        }
    }
    if !report.tokens_by_agent.is_empty() {
        out.push_str("\nTokens (state + bench jsonl by role prefix):\n");
        let mut rows: Vec<_> = report.tokens_by_agent.iter().collect();
        rows.sort_by(|a, b| b.1.cmp(a.1));
        for (role, n) in rows {
            out.push_str(&format!("- {role}: {n}\n"));
        }
    }
    out.push_str("\nRecommendations:\n");
    for rec in &report.recommendations {
        out.push_str(&format!(
            "[{}] {} — {}\n  → {}\n",
            rec.priority, rec.category, rec.summary, rec.action
        ));
    }
    if !applied.is_empty() {
        out.push_str("\nApplied:\n");
        for line in applied {
            out.push_str(&format!("- {line}\n"));
        }
    }
    out
}

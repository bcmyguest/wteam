//! Generic Goldfish phases: sequence configurable personas (fresh session each),
//! stamp pass/fail. Workflow brains live in roles; this module is state + turns.
//!
//! Presets (`plan-review`, `validate`) are persona lists only. New workflows =
//! new roles + a preset or `--persona`, not new engine code.

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::backup;
use crate::engine::Ctx;
use crate::pickup;
use crate::state;
use crate::transport::resume;
use crate::work;

/// Built-in persona chains. Unknown phase names require `--persona`.
pub fn preset_personas(phase: &str) -> Option<&'static [&'static str]> {
    match phase {
        "plan-review" => Some(&["spec-comprehension", "spec-critic", "spec-readiness"]),
        "validate" => Some(&["validate-local", "validate-mid", "validate-out"]),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VerdictKind {
    Pass,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseVerdict {
    pub kind: VerdictKind,
    pub persona: String,
    pub findings: Vec<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PersonaOutcome {
    pub persona: String,
    pub kind: VerdictKind,
    pub findings: Vec<String>,
    pub notes: Vec<String>,
    pub report: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct PhaseReport {
    pub phase: String,
    pub passed: bool,
    pub outcomes: Vec<PersonaOutcome>,
    pub stamp: Option<PathBuf>,
}

/// Stamp a Goldfish verdict for the active persona into `phase`'s pending file.
pub fn record_verdict(
    ctx: &Ctx,
    phase: &str,
    persona: &str,
    kind: VerdictKind,
    findings: Vec<String>,
    notes: Vec<String>,
) -> Result<PhaseVerdict> {
    if kind == VerdictKind::Fail && findings.is_empty() {
        bail!("phase-verdict fail needs at least one finding");
    }
    let v = PhaseVerdict {
        kind,
        persona: persona.to_string(),
        findings,
        notes,
    };
    let dir = ctx.paths.phase_dir(phase);
    fs::create_dir_all(&dir)?;
    let pending = ctx.paths.phase_pending_file(phase);
    let body = serde_json::to_string_pretty(&v)?;
    fs::write(&pending, format!("{body}\n"))
        .with_context(|| format!("writing {}", pending.display()))?;
    Ok(v)
}

/// Run `personas` for `phase` in order. Fresh session each. Stops at first FAIL.
pub fn run(ctx: &Ctx, phase: &str, personas: &[&str], ticket: Option<&str>) -> Result<PhaseReport> {
    if personas.is_empty() {
        bail!("phase `{phase}` needs at least one persona (pass --persona or use a preset)");
    }
    if backup::pending(&ctx.paths) {
        let _ = backup::restore(&ctx.paths)?;
    }
    // plan-review gates on a clean pickup; validate runs after code lands.
    if phase == "plan-review" {
        let report = pickup::pickup(&ctx.paths, None)?;
        if !report.problems.is_empty() {
            bail!(
                "pickup has problems; fix them before plan-review:\n{}",
                report.problems.join("\n")
            );
        }
    }
    let dir = ctx.paths.phase_dir(phase);
    fs::create_dir_all(&dir)?;
    let _ = fs::remove_file(ctx.paths.phase_passed_file(phase));

    let mut outcomes = Vec::new();
    for (i, persona) in personas.iter().enumerate() {
        let outcome = run_persona(ctx, phase, persona, i + 1, ticket)?;
        let failed = outcome.kind == VerdictKind::Fail;
        outcomes.push(outcome);
        if failed {
            break;
        }
    }
    let passed = outcomes.iter().all(|o| o.kind == VerdictKind::Pass);
    let stamp = if passed {
        Some(write_passed(ctx, phase, &outcomes)?)
    } else {
        None
    };
    Ok(PhaseReport {
        phase: phase.to_string(),
        passed,
        outcomes,
        stamp,
    })
}

fn run_persona(
    ctx: &Ctx,
    phase: &str,
    persona: &str,
    step: usize,
    ticket: Option<&str>,
) -> Result<PersonaOutcome> {
    let instance = format!("{persona}.1");
    let _ = fs::remove_file(ctx.paths.phase_pending_file(phase));
    let prompt = persona_prompt(ctx, phase, persona, step, ticket)?;
    let turns = state::load(&ctx.paths)?
        .agents
        .get(&instance)
        .map(|a| a.turns)
        .unwrap_or(0)
        + 1;
    if !resume::begin_foreground(&ctx.paths, &instance)? {
        bail!("{instance} is already busy");
    }
    let result = resume::one_turn_def(
        ctx,
        &instance,
        persona,
        true,
        None,
        turns,
        &prompt,
        true,
        false,
        Some(phase),
    );
    resume::end_foreground(&ctx.paths, &instance)?;
    result?;
    if backup::pending(&ctx.paths) {
        let _ = backup::restore(&ctx.paths)?;
    }

    let pending = ctx.paths.phase_pending_file(phase);
    if !pending.is_file() {
        bail!("{persona} ended without `wteam phase-verdict`");
    }
    let verdict: PhaseVerdict = serde_json::from_str(&fs::read_to_string(&pending)?)
        .with_context(|| format!("parsing {}", pending.display()))?;
    if verdict.persona != *persona {
        bail!(
            "phase-verdict persona `{}` != active `{persona}`",
            verdict.persona
        );
    }
    let report = ctx
        .paths
        .phase_dir(phase)
        .join(format!("{step:02}-{persona}.md"));
    fs::write(&report, format_report(&verdict))?;
    let _ = fs::remove_file(&pending);
    Ok(PersonaOutcome {
        persona: persona.to_string(),
        kind: verdict.kind,
        findings: verdict.findings,
        notes: verdict.notes,
        report,
    })
}

fn persona_prompt(
    ctx: &Ctx,
    phase: &str,
    persona: &str,
    step: usize,
    ticket: Option<&str>,
) -> Result<String> {
    let slug = &ctx.paths.slug;
    let phase_dir = format!(".wteam/{slug}/{}", ctx.paths.phase_rel(phase));
    let ids = work::load_tickets(&ctx.paths.tickets_dir())?
        .into_iter()
        .map(|t| t.id)
        .collect::<Vec<_>>()
        .join(", ");
    let body = match phase {
        "plan-review" => format!(
            "Read `spec.md` and every file under `tickets/` in `.wteam/{slug}/`.\n\
             Fresh session — no prior chat.\n\
             Ticket ids present: {ids}\n"
        ),
        "validate" => {
            let focus = match ticket {
                Some(t) => format!(
                    "Focus ticket `{t}` first (local zoom), then widen if your role says so.\n"
                ),
                None => "Zoom per your role: local tickets, then mid, then out.\n".into(),
            };
            format!(
                "Effort `.wteam/{slug}/` has implementation work on disk.\n\
                 {focus}\
                 Read `spec.md`, tickets, and the tree. Run Verify commands yourself.\n\
                 Fresh session — no prior chat.\n\
                 Ticket ids: {ids}\n"
            )
        }
        _ => format!(
            "Phase `{phase}` step for effort `.wteam/{slug}/`.\n\
             Fresh session. Ticket ids: {ids}\n"
        ),
    };
    Ok(format!(
        "Phase `{phase}` step {step}, persona `{persona}`.\n\
         {body}\
         Write findings under `{phase_dir}/`, then:\n\
         `wteam phase-verdict pass|fail --file <path>`\n\
         (`wteam plan-verdict` is an alias when phase is plan-review.)\n"
    ))
}

fn format_report(v: &PhaseVerdict) -> String {
    let mut s = format!("# {} — {:?}\n\nFINDINGS:\n", v.persona, v.kind);
    if v.findings.is_empty() {
        s.push_str("(none)\n");
    } else {
        for f in &v.findings {
            s.push_str(&format!("- {f}\n"));
        }
    }
    if !v.notes.is_empty() {
        s.push_str("\nNOTES:\n");
        for n in &v.notes {
            s.push_str(&format!("- {n}\n"));
        }
    }
    s
}

fn write_passed(ctx: &Ctx, phase: &str, outcomes: &[PersonaOutcome]) -> Result<PathBuf> {
    let path = ctx.paths.phase_passed_file(phase);
    let names: Vec<&str> = outcomes.iter().map(|o| o.persona.as_str()).collect();
    fs::write(
        &path,
        format!(
            "# Phase `{phase}` PASSED\n\npersonas: {}\n",
            names.join(", ")
        ),
    )?;
    Ok(path)
}

pub fn report_out(r: &PhaseReport) -> (String, serde_json::Value) {
    let mut lines = vec![format!(
        "phase {} — {}",
        r.phase,
        if r.passed { "PASSED" } else { "FAILED" }
    )];
    for o in &r.outcomes {
        lines.push(format!(
            "  {} {:?}: {} finding(s)",
            o.persona,
            o.kind,
            o.findings.len()
        ));
        for f in &o.findings {
            lines.push(format!("    - {f}"));
        }
    }
    if let Some(stamp) = &r.stamp {
        lines.push(format!("stamp: {}", stamp.display()));
    }
    (
        lines.join("\n"),
        json!({
            "phase": r.phase,
            "passed": r.passed,
            "outcomes": r.outcomes,
            "stamp": r.stamp.as_ref().map(|p| p.display().to_string()),
        }),
    )
}

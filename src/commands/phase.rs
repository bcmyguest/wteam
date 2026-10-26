//! `wteam phase` / `wteam phase-verdict` — general Goldfish common actions.
//! `plan-review` / `plan-verdict` remain aliases for the plan-review preset.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde_json::json;

use super::Out;
use crate::cli::{
    Global, PhaseArgs, PhaseVerdictArgs, PlanReviewArgs, PlanVerdictArgs, VerdictValue,
};
use crate::commands::tickets::{self, VerdictSections};
use crate::engine::Ctx;
use crate::phase::{self, VerdictKind};

pub fn phase(g: &Global, a: &PhaseArgs) -> Result<Out> {
    let ctx = Ctx::load(super::effort(g)?)?;
    let owned: Vec<String> = a.persona.clone();
    let preset = phase::preset_personas(&a.name);
    let personas: Vec<&str> = if owned.is_empty() {
        preset
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "unknown phase `{}` (no built-in personas); pass --persona",
                    a.name
                )
            })?
            .to_vec()
    } else {
        owned.iter().map(String::as_str).collect()
    };
    let report = phase::run(&ctx, &a.name, &personas, a.ticket.as_deref())?;
    let (text, value) = phase::report_out(&report);
    if !report.passed {
        bail!("{text}");
    }
    Ok(Out::new(text, value))
}

pub fn phase_verdict(g: &Global, a: &PhaseVerdictArgs) -> Result<Out> {
    let ctx = Ctx::load(super::effort(g)?)?;
    let phase_name = a
        .phase
        .clone()
        .or_else(|| std::env::var("WTEAM_PHASE").ok())
        .ok_or_else(|| {
            anyhow::anyhow!("phase-verdict needs --phase or WTEAM_PHASE (set by `wteam phase`)")
        })?;
    record(
        g,
        &ctx,
        &phase_name,
        a.verdict,
        &a.findings,
        &a.notes,
        &a.file,
    )
}

pub fn plan_review(g: &Global, a: &PlanReviewArgs) -> Result<Out> {
    phase(
        g,
        &PhaseArgs {
            name: "plan-review".into(),
            persona: a.persona.clone(),
            ticket: None,
        },
    )
}

pub fn plan_verdict(g: &Global, a: &PlanVerdictArgs) -> Result<Out> {
    let ctx = Ctx::load(super::effort(g)?)?;
    let phase_name = std::env::var("WTEAM_PHASE").unwrap_or_else(|_| "plan-review".into());
    record(
        g,
        &ctx,
        &phase_name,
        a.verdict,
        &a.findings,
        &a.notes,
        &a.file,
    )
}

fn record(
    g: &Global,
    ctx: &Ctx,
    phase_name: &str,
    verdict: VerdictValue,
    findings: &[String],
    notes: &[String],
    file: &Option<PathBuf>,
) -> Result<Out> {
    let persona = g
        .agent
        .as_deref()
        .map(|id| crate::team::parse_instance(id).0)
        .ok_or_else(|| anyhow::anyhow!("phase-verdict requires WTEAM_AGENT"))?;
    let mut sections = match file {
        Some(p) => tickets::parse_verdict_file(&read_file(p)?)
            .with_context(|| format!("verdict file `{}`", p.display()))?,
        None => VerdictSections::default(),
    };
    sections.findings.extend(findings.iter().cloned());
    sections.notes.extend(notes.iter().cloned());
    let kind = match verdict {
        VerdictValue::Pass => VerdictKind::Pass,
        VerdictValue::Fail => VerdictKind::Fail,
    };
    let v = phase::record_verdict(
        ctx,
        phase_name,
        &persona,
        kind,
        sections.findings,
        sections.notes,
    )?;
    Ok(Out::new(
        format!("recorded {:?} for {persona} ({phase_name})", v.kind).to_lowercase(),
        json!({
            "phase": phase_name,
            "persona": persona,
            "verdict": v.kind,
            "findings": v.findings,
            "notes": v.notes,
        }),
    ))
}

fn read_file(path: &PathBuf) -> Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("cannot read `{}`", path.display()))
}

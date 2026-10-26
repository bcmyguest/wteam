//! Plan-review preset: thin alias over [`crate::phase`] (`phase = plan-review`).

use anyhow::Result;

use crate::engine::Ctx;
use crate::phase::{self, PhaseReport, PhaseVerdict};

pub use phase::PersonaOutcome;
pub use phase::VerdictKind as PlanVerdictKind;

pub const DEFAULT_PERSONAS: [&str; 3] = ["spec-comprehension", "spec-critic", "spec-readiness"];

pub type PlanVerdict = PhaseVerdict;
pub type PlanReviewReport = PhaseReport;

pub fn record_verdict(
    ctx: &Ctx,
    persona: &str,
    kind: PlanVerdictKind,
    findings: Vec<String>,
    notes: Vec<String>,
) -> Result<PhaseVerdict> {
    phase::record_verdict(ctx, "plan-review", persona, kind, findings, notes)
}

pub fn run(ctx: &Ctx, personas: &[&str]) -> Result<PhaseReport> {
    phase::run(ctx, "plan-review", personas, None)
}

pub fn report_out(r: &PhaseReport) -> (String, serde_json::Value) {
    phase::report_out(r)
}

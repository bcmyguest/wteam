//! `wteam __turn` (internal).

use anyhow::Result;

use super::Out;
use crate::cli::{Global, TurnArgs};
use crate::engine::Ctx;

pub fn run(g: &Global, a: &TurnArgs) -> Result<Out> {
    let ctx = Ctx::load(super::effort(g)?)?;
    crate::transport::resume::run_turns(&ctx, &a.agent)?;
    Ok(Out::new(
        String::new(),
        serde_json::json!({ "agent": a.agent }),
    ))
}

//! `wteam report`.

use anyhow::Result;

use super::Out;
use crate::cli::Global;

pub fn run(g: &Global) -> Result<Out> {
    let p = super::effort(g)?;
    Ok(Out::new(
        crate::report::stop_report(&p)?,
        crate::report::report_data(&p)?,
    ))
}

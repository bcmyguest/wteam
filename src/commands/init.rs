//! `wteam init`.

use anyhow::Result;

use super::Out;
use crate::cli::Global;
use crate::paths::{self, Paths};

pub fn run(g: &Global) -> Result<Out> {
    let root = super::root(g)?;
    paths::init_wteam_dir(&root)?;
    let slug = g.slug.clone().unwrap_or_else(|| "main".to_string());
    let p = Paths::new(root, slug);
    paths::init_effort(&p)?;
    Ok(Out::new(
        format!("initialised {}", p.effort_dir().display()),
        serde_json::json!({ "effort": p.effort_dir(), "slug": p.slug }),
    ))
}

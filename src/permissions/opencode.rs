//! Translate a profile for opencode.

use anyhow::{Result, bail};

use super::{Profile, Translation};

pub fn translate(profile: &Profile, role: &str) -> Result<Translation> {
    let _ = (profile, role);
    bail!("not implemented: permissions::opencode::translate")
}

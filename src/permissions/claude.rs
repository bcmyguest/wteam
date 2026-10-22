//! Translate a profile for claude.

use anyhow::{Result, bail};

use super::{Profile, Translation};

pub fn translate(profile: &Profile, role: &str) -> Result<Translation> {
    let _ = (profile, role);
    bail!("not implemented: permissions::claude::translate")
}

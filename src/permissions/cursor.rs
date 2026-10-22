//! Translate a profile for cursor.

use anyhow::{Result, bail};

use super::{Profile, Translation};

pub fn translate(profile: &Profile, role: &str) -> Result<Translation> {
    let _ = (profile, role);
    bail!("not implemented: permissions::cursor::translate")
}

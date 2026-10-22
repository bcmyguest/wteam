//! Agent definitions: reusable TOML files naming a CLI, model, role prompt and
//! permission profile.
//!
//! ```toml
//! name = "qwen-worker"
//! cli = "opencode"            # claude | codex | opencode | cursor | fake
//! model = "lemonade/qwen3.8-flash"
//! prompt = "worker"           # built-in role prompt name, or a path
//! profile = "edit"            # permission profile name
//! extra_args = []
//! ```

use std::path::PathBuf;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentDef {
    pub name: String,
    pub cli: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub extra_args: Vec<String>,
}

/// Find agent `name` in `search` dirs (first hit wins), else the built-ins.
pub fn load(name: &str, search: &[PathBuf]) -> Result<AgentDef> {
    let _ = (name, search);
    bail!("not implemented: agents::load")
}

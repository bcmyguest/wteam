//! `wteam setup`.

use std::io::{IsTerminal, Write};

use anyhow::{Result, bail};
use serde_json::json;

use super::Out;
use crate::cli::SetupArgs;
use crate::config;

/// Lowest herdr version the herdr transport supports (`herdr agent prompt` and `agent start`).
pub const HERDR_MIN: (u64, u64, u64) = (0, 9, 5);

/// Install command shown to the user. Setup prints it and never runs it.
pub const HERDR_INSTALL: &str = "curl -fsSL https://herdr.dev/install.sh | sh";

/// Version triple from the first token that starts with a digit, e.g. `herdr 0.9.5` → `(0, 9, 5)`.
pub fn parse_version(out: &str) -> Option<(u64, u64, u64)> {
    let token = out
        .split_whitespace()
        .find(|t| t.starts_with(|c: char| c.is_ascii_digit()))?;
    let mut parts = token.split('.').map(|p| {
        p.chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse::<u64>()
            .ok()
    });
    Some((
        parts.next()??,
        parts.next()??,
        parts.next().flatten().unwrap_or(0),
    ))
}

fn fmt_version(v: (u64, u64, u64)) -> String {
    format!("{}.{}.{}", v.0, v.1, v.2)
}

/// Version reported by `herdr --version`, if herdr is on PATH and runs.
fn herdr_version() -> Option<(u64, u64, u64)> {
    let out = std::process::Command::new("herdr")
        .arg("--version")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    parse_version(&String::from_utf8_lossy(&out.stdout))
}

/// Ask a yes/no question on stdin; anything but `n` counts as yes.
fn ask(question: &str) -> Result<bool> {
    eprint!("{question} [Y/n] ");
    std::io::stderr().flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    Ok(!line.trim().to_lowercase().starts_with('n'))
}

pub fn run(a: &SetupArgs) -> Result<Out> {
    let found = herdr_version();
    let herdr_ok = found.is_some_and(|v| v >= HERDR_MIN);
    let mut text = match found {
        Some(v) if herdr_ok => format!("herdr {} detected\n", fmt_version(v)),
        Some(v) => format!(
            "herdr {} is older than {} (need {} or newer)\n",
            fmt_version(v),
            fmt_version(HERDR_MIN),
            fmt_version(HERDR_MIN)
        ),
        None => "herdr not found on PATH\n".to_string(),
    };
    if !herdr_ok {
        text.push_str(&format!("install herdr with:\n  {HERDR_INSTALL}\n"));
    }
    if herdr_ok {
        text.push_str(&format!(
            "herdr agents start in the pane named by {}: set it to an idle shell pane id\n",
            crate::transport::herdr::PANE_ENV
        ));
    }

    let interactive = std::io::stdin().is_terminal() && !a.yes;
    let transport = match (&a.transport, interactive) {
        (Some(forced), _) => {
            crate::transport::for_name(forced)?;
            forced.clone()
        }
        (None, false) => if herdr_ok { "herdr" } else { "resume" }.to_string(),
        (None, true) if herdr_ok => {
            if ask("use the herdr transport?")? {
                "herdr".to_string()
            } else {
                "resume".to_string()
            }
        }
        (None, true) => {
            if !ask("herdr is missing; continue with the resume transport?")? {
                bail!("no transport chosen");
            }
            "resume".to_string()
        }
    };

    let file = config::set_transport(&transport)?;
    text.push_str(&format!(
        "wrote {}: transport = \"{transport}\"",
        file.display()
    ));
    let value = json!({
        "transport": transport,
        "config": file,
        "herdr_version": found.map(fmt_version),
        "herdr_ok": herdr_ok,
        "install": (!herdr_ok).then_some(HERDR_INSTALL),
    });
    Ok(Out { text, json: value })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_herdr_version_output() {
        assert_eq!(parse_version("herdr 0.9.5\n"), Some((0, 9, 5)));
        assert_eq!(parse_version("herdr 1.2"), Some((1, 2, 0)));
        assert_eq!(parse_version("herdr 0.10.0-rc1"), Some((0, 10, 0)));
        assert_eq!(parse_version("no version here"), None);
        assert!(parse_version("herdr 0.9.4").unwrap() < HERDR_MIN);
        assert!(parse_version("herdr 0.9.5").unwrap() >= HERDR_MIN);
    }
}

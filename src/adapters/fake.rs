//! The fake adapter: runs `sh $WTEAM_FAKE_DIR/<role>.sh` for tests.

use anyhow::{Result, bail};

use super::{Adapter, CommandSpec, TurnOutput, TurnRequest};
use crate::team::parse_instance;

pub struct Fake;

impl Adapter for Fake {
    fn name(&self) -> &'static str {
        "fake"
    }

    fn command(&self, req: &TurnRequest) -> CommandSpec {
        let (role, _) = parse_instance(&req.agent_id);
        let dir = req
            .env
            .iter()
            .find(|(k, _)| k == "WTEAM_FAKE_DIR")
            .map(|(_, v)| v.clone())
            .or_else(|| std::env::var("WTEAM_FAKE_DIR").ok())
            .unwrap_or_else(|| ".".into());
        let mut env = vec![
            ("WTEAM_PROMPT".to_string(), req.prompt.clone()),
            (
                "WTEAM_SESSION".to_string(),
                req.session_id.clone().unwrap_or_default(),
            ),
        ];
        env.extend(req.env.iter().cloned());
        CommandSpec {
            program: "sh".into(),
            args: vec![format!("{dir}/{role}.sh")],
            env,
            cwd: req.cwd.clone(),
            stdin: None,
        }
    }

    /// `session:` and `tokens:` lines are metadata; the rest is text. A missing
    /// session stays `None`: the turn runner invents `fake-<agent>`.
    fn parse_output(&self, stdout: &str) -> Result<TurnOutput> {
        let mut out = TurnOutput::default();
        let mut text = Vec::new();
        for line in stdout.lines() {
            if let Some(v) = line.strip_prefix("session:") {
                out.session_id = Some(v.trim().to_string());
            } else if let Some(v) = line.strip_prefix("tokens:") {
                match v.trim().parse() {
                    Ok(n) => out.context_tokens = Some(n),
                    Err(_) => bail!("bad tokens line: {line}"),
                }
            } else {
                text.push(line);
            }
        }
        out.text = text.join("\n").trim().to_string();
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_runs_role_script_with_env() {
        let req = TurnRequest {
            agent_id: "worker.2".into(),
            prompt: "do it".into(),
            session_id: Some("s1".into()),
            env: vec![
                ("WTEAM_FAKE_DIR".into(), "/fk".into()),
                ("WTEAM_AGENT".into(), "worker.2".into()),
            ],
            ..Default::default()
        };
        let c = Fake.command(&req);
        assert_eq!(c.program, "sh");
        assert_eq!(c.args, vec!["/fk/worker.sh"]);
        assert!(c.env.contains(&("WTEAM_PROMPT".into(), "do it".into())));
        assert!(c.env.contains(&("WTEAM_SESSION".into(), "s1".into())));
        assert!(c.env.contains(&("WTEAM_AGENT".into(), "worker.2".into())));
        let first = TurnRequest {
            session_id: None,
            ..req
        };
        assert!(
            Fake.command(&first)
                .env
                .contains(&("WTEAM_SESSION".into(), String::new()))
        );
    }

    #[test]
    fn parse_reads_session_tokens_and_text() {
        let o = Fake
            .parse_output("hello\nsession: abc\ntokens: 1234\nbye\n")
            .unwrap();
        assert_eq!(o.session_id.as_deref(), Some("abc"));
        assert_eq!(o.context_tokens, Some(1234));
        assert_eq!(o.text, "hello\nbye");
        let none = Fake.parse_output("just text").unwrap();
        assert_eq!(none.session_id, None);
        assert!(Fake.parse_output("tokens: lots").is_err());
    }
}

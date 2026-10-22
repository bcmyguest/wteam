//! Messaging: send, inbox.

use anyhow::Result;

use super::Out;
use crate::cli::{Global, InboxArgs, SendArgs};
use crate::engine::{self, Ctx};
use crate::mailbox;
use crate::state;
use crate::team::{instance_id, parse_instance};

pub fn send(g: &Global, a: &SendArgs) -> Result<Out> {
    let ctx = Ctx::load(super::effort(g)?)?;
    let from = super::caller(g, "human");
    engine::send(&ctx, &from, &a.to, &a.message)?;
    Ok(Out {
        text: format!("sent to {}", a.to),
        json: serde_json::json!({ "from": from, "to": a.to }),
    })
}

pub fn inbox(g: &Global, a: &InboxArgs) -> Result<Out> {
    let paths = super::effort(g)?;
    let raw = super::caller(g, "human");
    let (role, index) = parse_instance(&raw);
    let me = if raw == "human" {
        raw
    } else {
        instance_id(&role, index)
    };
    let text = state::with_state(&paths, |s| {
        let st = s.agents.entry(me.clone()).or_default();
        if st.role.is_empty() {
            st.role = role.clone();
            st.index = index;
        }
        let start = if a.all { 0 } else { st.mail_read };
        let (text, end) = mailbox::read_from(&paths, &me, start)?;
        st.mail_read = st.mail_read.max(end);
        Ok(text)
    })?;
    let msgs = mailbox::parse(&text);
    let human = if msgs.is_empty() {
        "no messages".to_string()
    } else {
        msgs.iter()
            .map(|m| {
                let t = m
                    .ticket
                    .as_deref()
                    .map(|t| format!(" [{t}]"))
                    .unwrap_or_default();
                format!("from {} at {}{t}\n{}", m.from, m.at, m.body)
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    };
    Ok(Out {
        text: human,
        json: serde_json::json!({ "agent": me, "messages": msgs }),
    })
}

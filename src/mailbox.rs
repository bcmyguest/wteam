//! Mailboxes: one append-only markdown file per agent instance under `mail/`.
//! The file is the record; a transport delivers new bytes past
//! `AgentState::mail_delivered`.
//!
//! Each message is `---`, a header (`from:`, `at:`, optional `ticket:`), a blank
//! line, then the body.

use std::io::{Read, Seek, SeekFrom, Write};

use anyhow::{Context, Result};
use serde::Serialize;

use crate::paths::Paths;

/// One parsed mailbox message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Message {
    pub from: String,
    pub at: u64,
    pub ticket: Option<String>,
    pub body: String,
}

/// Append a message to `to`'s mailbox. Returns the new file length.
pub fn append(paths: &Paths, to: &str, from: &str, body: &str) -> Result<u64> {
    append_ticket(paths, to, from, None, body)
}

/// Like [`append`], tagging the message with a ticket id.
pub fn append_ticket(
    paths: &Paths,
    to: &str,
    from: &str,
    ticket: Option<&str>,
    body: &str,
) -> Result<u64> {
    std::fs::create_dir_all(paths.mail_dir())
        .with_context(|| format!("creating {}", paths.mail_dir().display()))?;
    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut msg = format!("---\nfrom: {from}\nat: {at}\n");
    if let Some(t) = ticket {
        msg.push_str(&format!("ticket: {t}\n"));
    }
    msg.push_str(&format!("\n{}\n\n", body.trim_end()));
    let file = paths.mail_file(to);
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file)
        .with_context(|| format!("opening {}", file.display()))?;
    // One write call keeps concurrent appends from interleaving.
    f.write_all(msg.as_bytes())?;
    Ok(f.metadata()?.len())
}

/// Mailbox contents from byte `offset` to the end, and the end offset.
pub fn read_from(paths: &Paths, agent: &str, offset: u64) -> Result<(String, u64)> {
    let file = paths.mail_file(agent);
    let mut f = match std::fs::File::open(&file) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((String::new(), offset)),
        Err(e) => return Err(e).with_context(|| format!("opening {}", file.display())),
    };
    f.seek(SeekFrom::Start(offset))?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    Ok((
        String::from_utf8_lossy(&buf).into_owned(),
        offset + buf.len() as u64,
    ))
}

/// Parse mailbox text into messages.
pub fn parse(text: &str) -> Vec<Message> {
    let lines: Vec<&str> = text.lines().collect();
    let starts: Vec<usize> = (0..lines.len())
        .filter(|&i| lines[i] == "---" && lines.get(i + 1).is_some_and(|l| l.starts_with("from: ")))
        .collect();
    let mut out = Vec::new();
    for (n, &s) in starts.iter().enumerate() {
        let end = starts.get(n + 1).copied().unwrap_or(lines.len());
        let mut msg = Message {
            from: String::new(),
            at: 0,
            ticket: None,
            body: String::new(),
        };
        let mut i = s + 1;
        while i < end && !lines[i].is_empty() {
            if let Some(v) = lines[i].strip_prefix("from: ") {
                msg.from = v.trim().to_string();
            } else if let Some(v) = lines[i].strip_prefix("at: ") {
                msg.at = v.trim().parse().unwrap_or(0);
            } else if let Some(v) = lines[i].strip_prefix("ticket: ") {
                msg.ticket = Some(v.trim().to_string());
            }
            i += 1;
        }
        msg.body = lines[(i + 1).min(end)..end].join("\n").trim().to_string();
        out.push(msg);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_creates_dir_and_read_from_resumes() {
        let tmp = tempfile::tempdir().unwrap();
        let p = Paths::new(tmp.path(), "s");
        let n1 = append(&p, "worker.1", "human", "hello").unwrap();
        let (text, end) = read_from(&p, "worker.1", 0).unwrap();
        assert_eq!(end, n1);
        let n2 = append_ticket(&p, "worker.1", "orchestrator", Some("03"), "two\nlines").unwrap();
        let (rest, end2) = read_from(&p, "worker.1", n1).unwrap();
        assert_eq!(end2, n2);
        assert!(!rest.contains("hello") && text.contains("hello"));
        let all = parse(&format!("{text}{rest}"));
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].from, "human");
        assert_eq!(all[0].body, "hello");
        assert_eq!(all[0].ticket, None);
        assert_eq!(all[1].ticket.as_deref(), Some("03"));
        assert_eq!(all[1].body, "two\nlines");
    }

    #[test]
    fn missing_mailbox_reads_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let p = Paths::new(tmp.path(), "s");
        assert_eq!(read_from(&p, "nobody.1", 0).unwrap(), (String::new(), 0));
    }

    #[test]
    fn concurrent_appends_do_not_interleave() {
        let tmp = tempfile::tempdir().unwrap();
        let p = Paths::new(tmp.path(), "s");
        let handles: Vec<_> = (0..8)
            .map(|t| {
                let p = p.clone();
                std::thread::spawn(move || {
                    for i in 0..25 {
                        let body = format!("t{t}-{i} {}", "x".repeat(2000));
                        append(&p, "a.1", &format!("w.{t}"), &body).unwrap();
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        let (text, _) = read_from(&p, "a.1", 0).unwrap();
        let msgs = parse(&text);
        assert_eq!(msgs.len(), 200);
        for m in msgs {
            assert!(m.body.starts_with('t') && m.body.ends_with('x'), "{m:?}");
            assert_eq!(m.body.matches(' ').count(), 1);
        }
    }
}

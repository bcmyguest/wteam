//! Mailboxes: one append-only markdown file per agent instance under `mail/`.
//! The file is the record; a transport delivers new bytes past
//! `AgentState::mail_delivered`.
//!
//! Each message is the marker line [`MARKER`], a header (`from:`, `at:`, optional
//! `ticket:`, `length:`), a blank line, then the body. `length:` is the body's
//! byte length, so [`parse`] skips the body without scanning it: a body may
//! contain `---`, `from: x` or even a marker line without splitting the message.

use std::io::{Read, Seek, SeekFrom, Write};

use anyhow::{Context, Result};
use serde::Serialize;

use crate::paths::Paths;

/// First line of every message.
const MARKER: &str = "--- wteam-mail ---";

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
    let body = body.trim_end();
    let mut msg = format!("{MARKER}\nfrom: {from}\nat: {at}\n");
    if let Some(t) = ticket {
        msg.push_str(&format!("ticket: {t}\n"));
    }
    msg.push_str(&format!("length: {}\n\n{body}\n\n", body.len()));
    let file = paths.mail_file(to);
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file)
        .with_context(|| format!("opening {}", file.display()))?;
    // The exclusive lock keeps readers from seeing a half-written body.
    f.lock()
        .with_context(|| format!("locking {}", file.display()))?;
    f.write_all(msg.as_bytes())?;
    f.unlock().ok();
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
    f.lock_shared()
        .with_context(|| format!("locking {}", file.display()))?;
    f.seek(SeekFrom::Start(offset))?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    f.unlock().ok();
    Ok((
        String::from_utf8_lossy(&buf).into_owned(),
        offset + buf.len() as u64,
    ))
}

/// Parse mailbox text into messages. Bodies are cut by their `length:` header;
/// a message without one ends at the next marker line.
pub fn parse(text: &str) -> Vec<Message> {
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(start) = next_marker(text, pos) {
        let mut msg = Message {
            from: String::new(),
            at: 0,
            ticket: None,
            body: String::new(),
        };
        let mut length = None;
        let mut cur = start + MARKER.len() + 1;
        while cur < text.len() {
            let end = text[cur..].find('\n').map_or(text.len(), |i| cur + i);
            let line = &text[cur..end];
            cur = (end + 1).min(text.len());
            if line.is_empty() {
                break;
            }
            if let Some(v) = line.strip_prefix("from: ") {
                msg.from = v.trim().to_string();
            } else if let Some(v) = line.strip_prefix("at: ") {
                msg.at = v.trim().parse().unwrap_or(0);
            } else if let Some(v) = line.strip_prefix("ticket: ") {
                msg.ticket = Some(v.trim().to_string());
            } else if let Some(v) = line.strip_prefix("length: ") {
                length = v.trim().parse::<usize>().ok();
            }
        }
        let rest = &text[cur..];
        let body = match length.and_then(|n| rest.get(..n)) {
            Some(b) => b,
            None => &rest[..next_marker(rest, 0).unwrap_or(rest.len())],
        };
        msg.body = body.trim().to_string();
        pos = cur + body.len();
        out.push(msg);
    }
    out
}

/// Byte index of the next marker line at or after `from`.
fn next_marker(text: &str, from: usize) -> Option<usize> {
    let mut at = from;
    while at < text.len() {
        let end = text[at..].find('\n').map_or(text.len(), |i| at + i);
        if &text[at..end] == MARKER {
            return Some(at);
        }
        at = end + 1;
    }
    None
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
    fn body_with_header_lookalikes_stays_one_message() {
        let tmp = tempfile::tempdir().unwrap();
        let p = Paths::new(tmp.path(), "s");
        let nasty =
            "diff:\n---\nfrom: x\nat: 5\n\nmore\n--- wteam-mail ---\nfrom: y\nlength: 3\n\nend";
        append(&p, "a.1", "w.1", nasty).unwrap();
        append(&p, "a.1", "w.2", "after").unwrap();
        let (text, _) = read_from(&p, "a.1", 0).unwrap();
        let msgs = parse(&text);
        assert_eq!(msgs.len(), 2, "{msgs:?}");
        assert_eq!(msgs[0].from, "w.1");
        assert_eq!(msgs[0].body, nasty);
        assert_eq!(msgs[1].body, "after");
    }

    #[test]
    fn multibyte_bodies_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let p = Paths::new(tmp.path(), "s");
        append(&p, "a.1", "w.1", "héllo ✓").unwrap();
        append(&p, "a.1", "w.1", "next").unwrap();
        let (text, _) = read_from(&p, "a.1", 0).unwrap();
        let msgs = parse(&text);
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].body, "héllo ✓");
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

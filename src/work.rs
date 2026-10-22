//! The work graph: tickets in the `/to-tickets` local format, plus readiness.
//!
//! Ticket file shape (`tickets/NN-<slug>.md`):
//!
//! ```markdown
//! # 03: Title
//!
//! **What to build:** prose
//!
//! **Blocked by:** 01, 02        (or "None (can start immediately)")
//!
//! **Status:** ready-for-agent
//!
//! **Files:** src/a.rs, src/b/**   (optional, wteam extension)
//!
//! **Verify:** cargo test -p x     (optional, wteam extension; `;`-separated)
//!
//! - [ ] Acceptance criterion
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use globset::Glob;
use serde::Serialize;

use crate::state::{State, TicketStatus};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ticket {
    /// Two-digit id from the heading, e.g. `03`.
    pub id: String,
    pub title: String,
    pub what: String,
    /// Ids of blocking tickets.
    pub blocked_by: Vec<String>,
    pub status_label: String,
    pub files: Vec<String>,
    pub verify: Vec<String>,
    /// `(checked, text)` acceptance criteria.
    pub criteria: Vec<(bool, String)>,
    pub path: PathBuf,
    /// Full markdown, used verbatim in worker briefs.
    pub body: String,
}

/// Which `**Label:**` block the parser is currently filling.
#[derive(Clone, Copy, PartialEq)]
enum Field {
    None,
    What,
    BlockedBy,
    Status,
    Files,
    Verify,
}

/// Split `**Label:** rest` into the label and the rest of the line.
fn split_label(line: &str) -> Option<(&str, &str)> {
    let rest = line.strip_prefix("**")?;
    let end = rest.find(":**")?;
    Some((&rest[..end], rest[end + 3..].trim()))
}

/// Normalise a ticket reference (`1`, `#01`) to its two-digit form.
fn normalize_id(raw: &str) -> Option<String> {
    let t = raw.trim().trim_start_matches('#');
    if t.is_empty() || !t.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(format!("{t:0>2}"))
}

/// Ids from a `Blocked by:` value: `01, 02`, `#1 2`, `01: Title`, or prose like "None (...)".
fn parse_blockers(value: &str) -> Vec<String> {
    let mut out = Vec::new();
    for seg in value.split(',') {
        let mut tokens = seg.split_whitespace();
        let Some(first) = tokens.next() else { continue };
        if let Some(id) = first.strip_suffix(':') {
            out.extend(normalize_id(id));
            continue;
        }
        for tok in std::iter::once(first).chain(tokens) {
            out.extend(normalize_id(tok));
        }
    }
    out
}

fn split_list(value: &str, sep: char) -> Vec<String> {
    value
        .split(sep)
        .map(|s| s.trim().trim_matches('`').trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Parse one ticket file's markdown. `path` is recorded, not read.
pub fn parse_ticket(markdown: &str, path: &Path) -> Result<Ticket> {
    let mut id = None;
    let mut title = String::new();
    let (mut what, mut blocked, mut status, mut files, mut verify) = (
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
    );
    let mut criteria = Vec::new();
    let mut field = Field::None;
    let mut in_criteria = false;

    for raw in markdown.lines() {
        let line = raw.trim_end();
        if id.is_none() {
            if let Some(h) = line.strip_prefix("# ") {
                let (num, rest) = match h.split_once(':') {
                    Some((n, r)) => (n, r),
                    None => h.split_once(' ').unwrap_or((h, "")),
                };
                id = normalize_id(num);
                if id.is_none() {
                    bail!(
                        "{}: heading `{line}` does not start with a ticket id",
                        path.display()
                    );
                }
                title = rest.trim().to_string();
            }
            continue;
        }
        let trimmed = line.trim_start();
        if let Some(item) = trimmed
            .strip_prefix("- [ ] ")
            .map(|t| (false, t))
            .or_else(|| trimmed.strip_prefix("- [x] ").map(|t| (true, t)))
            .or_else(|| trimmed.strip_prefix("- [X] ").map(|t| (true, t)))
        {
            in_criteria = true;
            criteria.push((item.0, item.1.trim().to_string()));
            continue;
        }
        if in_criteria {
            continue;
        }
        if let Some((label, rest)) = split_label(line) {
            field = match label.to_lowercase().as_str() {
                "what to build" => Field::What,
                "blocked by" => Field::BlockedBy,
                "status" => Field::Status,
                "files" => Field::Files,
                "verify" => Field::Verify,
                _ => Field::None,
            };
            let target = match field {
                Field::What => &mut what,
                Field::BlockedBy => &mut blocked,
                Field::Status => &mut status,
                Field::Files => &mut files,
                Field::Verify => &mut verify,
                Field::None => continue,
            };
            target.push_str(rest);
            continue;
        }
        let target = match field {
            Field::What => &mut what,
            Field::BlockedBy => &mut blocked,
            Field::Status => &mut status,
            Field::Files => &mut files,
            Field::Verify => &mut verify,
            Field::None => continue,
        };
        if !line.trim().is_empty() {
            if !target.is_empty() {
                target.push('\n');
            }
            target.push_str(line.trim());
        }
    }

    let Some(id) = id else {
        bail!("{}: no `# NN: Title` heading", path.display());
    };
    Ok(Ticket {
        id,
        title,
        what,
        blocked_by: parse_blockers(&blocked),
        status_label: status,
        files: split_list(&files, ','),
        verify: split_list(&verify, ';'),
        criteria,
        path: path.to_path_buf(),
        body: markdown.to_string(),
    })
}

/// Order ticket ids numerically: `02` < `10` < `100`.
fn id_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let (a, b) = (a.trim_start_matches('0'), b.trim_start_matches('0'));
    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}

/// Load every `*.md` ticket in `dir`, sorted by numeric id.
pub fn load_tickets(dir: &Path) -> Result<Vec<Ticket>> {
    let mut tickets = Vec::new();
    let rd = std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))?;
    for entry in rd {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let md = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        tickets.push(parse_ticket(&md, &path)?);
    }
    tickets.sort_by(|a, b| id_cmp(&a.id, &b.id).then_with(|| a.path.cmp(&b.path)));
    Ok(tickets)
}

/// Problems that make a work graph unrunnable.
pub fn validate(tickets: &[Ticket]) -> Vec<String> {
    let mut problems = Vec::new();
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for t in tickets {
        *seen.entry(t.id.as_str()).or_default() += 1;
    }
    for (id, n) in &seen {
        if *n > 1 {
            problems.push(format!("duplicate ticket id {id} ({n} files)"));
        }
    }
    for t in tickets {
        for b in &t.blocked_by {
            if !seen.contains_key(b.as_str()) {
                problems.push(format!("ticket {} is blocked by unknown ticket {b}", t.id));
            }
        }
    }
    problems.extend(find_cycles(tickets));
    problems
}

/// One message per ticket that sits on a dependency cycle (depth-first colouring).
fn find_cycles(tickets: &[Ticket]) -> Vec<String> {
    let mut deps: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for t in tickets {
        deps.entry(t.id.as_str())
            .or_default()
            .extend(t.blocked_by.iter().map(String::as_str));
    }
    let mut state: BTreeMap<&str, u8> = BTreeMap::new(); // 1 = on stack, 2 = done
    let mut cycles: Vec<Vec<String>> = Vec::new();
    let mut stack: Vec<&str> = Vec::new();

    fn visit<'a>(
        n: &'a str,
        deps: &BTreeMap<&'a str, BTreeSet<&'a str>>,
        state: &mut BTreeMap<&'a str, u8>,
        stack: &mut Vec<&'a str>,
        cycles: &mut Vec<Vec<String>>,
    ) {
        state.insert(n, 1);
        stack.push(n);
        for &m in deps.get(n).into_iter().flatten() {
            match state.get(m) {
                Some(1) => {
                    if let Some(pos) = stack.iter().position(|&x| x == m) {
                        cycles.push(stack[pos..].iter().map(|s| s.to_string()).collect());
                    }
                }
                Some(_) => {}
                None if deps.contains_key(m) => visit(m, deps, state, stack, cycles),
                None => {}
            }
        }
        stack.pop();
        state.insert(n, 2);
    }

    for &n in deps.keys() {
        if !state.contains_key(n) {
            visit(n, &deps, &mut state, &mut stack, &mut cycles);
        }
    }
    cycles
        .into_iter()
        .map(|c| {
            let mut path = c.clone();
            path.push(c[0].clone());
            format!("dependency cycle: {}", path.join(" -> "))
        })
        .collect()
}

/// Tickets whose blockers are all `done` and which are themselves `todo`.
pub fn ready<'a>(tickets: &'a [Ticket], state: &State) -> Vec<&'a Ticket> {
    let status = |id: &str| {
        state
            .tickets
            .get(id)
            .map(|t| t.status)
            .unwrap_or(TicketStatus::Todo)
    };
    let mut out: Vec<&Ticket> = tickets
        .iter()
        .filter(|t| status(&t.id) == TicketStatus::Todo)
        .filter(|t| {
            t.blocked_by
                .iter()
                .all(|b| tickets.iter().any(|x| &x.id == b) && status(b) == TicketStatus::Done)
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// The literal directory prefix of a glob, up to its first wildcard.
fn static_prefix(pattern: &str) -> &str {
    let cut = pattern.find(['*', '?', '[', '{']).unwrap_or(pattern.len());
    let head = &pattern[..cut];
    if cut == pattern.len() {
        return head;
    }
    head.rfind('/').map_or("", |i| &head[..=i])
}

fn globs_overlap(a: &str, b: &str) -> bool {
    let (Ok(ga), Ok(gb)) = (Glob::new(a), Glob::new(b)) else {
        return true;
    };
    if ga.compile_matcher().is_match(b) || gb.compile_matcher().is_match(a) {
        return true;
    }
    let (pa, pb) = (static_prefix(a), static_prefix(b));
    let wild = |p: &str, s: &str| p.len() != s.len();
    (wild(pa, a) || wild(pb, b)) && (Path::new(pa).starts_with(pb) || Path::new(pb).starts_with(pa))
}

/// True when two tickets' `files` globs could touch the same path. Tickets with
/// no `files` overlap everything.
pub fn files_overlap(a: &Ticket, b: &Ticket) -> bool {
    if a.files.is_empty() || b.files.is_empty() {
        return true;
    }
    a.files
        .iter()
        .any(|x| b.files.iter().any(|y| globs_overlap(x, y)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEMPLATE: &str = "# 03: Add the thing\n\n**What to build:** A thing.\nSecond line.\n\n**Blocked by:** 01, 02\n\n**Status:** ready-for-agent\n\n**Files:** src/a.rs, `src/b/**`\n\n**Verify:** cargo test -p x; cargo clippy\n\n- [ ] first\n- [x] second\n";

    fn parse(md: &str) -> Ticket {
        parse_ticket(md, Path::new("t.md")).unwrap()
    }

    fn t(id: &str, blocked: &[&str], files: &[&str]) -> Ticket {
        Ticket {
            id: id.into(),
            title: format!("T{id}"),
            what: String::new(),
            blocked_by: blocked.iter().map(|s| s.to_string()).collect(),
            status_label: String::new(),
            files: files.iter().map(|s| s.to_string()).collect(),
            verify: vec![],
            criteria: vec![],
            path: PathBuf::from(format!("{id}.md")),
            body: String::new(),
        }
    }

    fn done(state: &mut State, id: &str) {
        state.ticket_mut(id).status = TicketStatus::Done;
    }

    #[test]
    fn ids_sort_numerically() {
        let tmp = tempfile::tempdir().unwrap();
        for id in ["100", "10", "2", "11"] {
            let md = format!("# {id}: T{id}\n\n**What to build:** x.\n\n- [ ] ok\n");
            std::fs::write(tmp.path().join(format!("{id}-t.md")), md).unwrap();
        }
        let ids: Vec<String> = load_tickets(tmp.path())
            .unwrap()
            .into_iter()
            .map(|t| t.id)
            .collect();
        assert_eq!(ids, ["02", "10", "11", "100"]);
    }

    #[test]
    fn parses_full_template() {
        let k = parse(TEMPLATE);
        assert_eq!(k.id, "03");
        assert_eq!(k.title, "Add the thing");
        assert_eq!(k.what, "A thing.\nSecond line.");
        assert_eq!(k.blocked_by, ["01", "02"]);
        assert_eq!(k.status_label, "ready-for-agent");
        assert_eq!(k.files, ["src/a.rs", "src/b/**"]);
        assert_eq!(k.verify, ["cargo test -p x", "cargo clippy"]);
        assert_eq!(
            k.criteria,
            [(false, "first".to_string()), (true, "second".to_string())]
        );
        assert_eq!(k.body, TEMPLATE);
        assert_eq!(k.path, Path::new("t.md"));
    }

    #[test]
    fn none_means_no_blockers() {
        let k = parse("# 01: A\n\n**Blocked by:** None (can start immediately)\n");
        assert!(k.blocked_by.is_empty());
    }

    #[test]
    fn blocker_spellings() {
        for v in ["01 02", "1, 2", "#01, #02", "01: Title, 02: Other", "#1 #2"] {
            assert_eq!(parse_blockers(v), ["01", "02"], "{v}");
        }
    }

    #[test]
    fn optional_lines_may_be_missing() {
        let k = parse("# 02: B\n\n**What to build:** x\n\n**Blocked by:** 01\n\n- [ ] ok\n");
        assert!(k.files.is_empty());
        assert!(k.verify.is_empty());
        assert_eq!(k.status_label, "");
        assert_eq!(k.blocked_by, ["01"]);
    }

    #[test]
    fn missing_heading_is_an_error() {
        assert!(parse_ticket("no heading\n", Path::new("x.md")).is_err());
        assert!(parse_ticket("# Title only\n", Path::new("x.md")).is_err());
    }

    #[test]
    fn validate_clean_graph() {
        assert!(validate(&[t("01", &[], &[]), t("02", &["01"], &[])]).is_empty());
    }

    #[test]
    fn validate_reports_unknown_duplicate_and_cycle() {
        let msgs = validate(&[t("01", &["09"], &[]), t("01", &[], &[])]);
        assert!(
            msgs.iter()
                .any(|m| m.contains("duplicate") && m.contains("01"))
        );
        assert!(
            msgs.iter()
                .any(|m| m.contains("unknown") && m.contains("09"))
        );
        let msgs = validate(&[t("01", &["02"], &[]), t("02", &["01"], &[])]);
        assert!(
            msgs.iter()
                .any(|m| m.contains("cycle") && m.contains("01") && m.contains("02"))
        );
        let msgs = validate(&[t("05", &["05"], &[])]);
        assert!(msgs.iter().any(|m| m.contains("cycle") && m.contains("05")));
    }

    #[test]
    fn ready_needs_todo_and_done_blockers() {
        let ts = [
            t("03", &["01"], &[]),
            t("01", &[], &[]),
            t("02", &["01"], &[]),
        ];
        let mut s = State::new("s");
        let ids = |s: &State| {
            ready(&ts, s)
                .iter()
                .map(|t| t.id.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(&s), ["01"]);
        done(&mut s, "01");
        assert_eq!(ids(&s), ["02", "03"]);
        s.ticket_mut("02").status = TicketStatus::Doing;
        assert_eq!(ids(&s), ["03"]);
    }

    #[test]
    fn ready_ignores_unknown_blocker() {
        let ts = [t("01", &["09"], &[])];
        assert!(ready(&ts, &State::new("s")).is_empty());
    }

    #[test]
    fn overlap_rules() {
        let o = |a: &[&str], b: &[&str]| files_overlap(&t("01", &[], a), &t("02", &[], b));
        assert!(o(&[], &["src/a.rs"]));
        assert!(o(&["src/a.rs"], &[]));
        assert!(o(&["src/a.rs"], &["src/a.rs"]));
        assert!(o(&["src/**"], &["src/a.rs"]));
        assert!(o(&["src/a.rs"], &["src/**"]));
        assert!(o(&["src/**/x.rs"], &["src/a/*.rs"]));
        assert!(!o(&["src/a.rs"], &["src/b.rs"]));
        assert!(!o(&["src/a/**"], &["src/b/**"]));
        assert!(!o(&["src/a.rs", "docs/**"], &["src/b.rs", "tests/*.rs"]));
        assert!(o(&["src/a.rs", "docs/**"], &["README.md", "docs/x.md"]));
    }
}

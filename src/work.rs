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
pub fn normalize_id(raw: &str) -> Option<String> {
    let t = raw.trim().trim_start_matches('#');
    if t.is_empty() || !t.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(format!("{t:0>2}"))
}

/// Ids from a `Blocked by:` value: `01, 02`, `#1 2`, `01: Title`, or prose like "None (...)".
pub fn parse_blockers(value: &str) -> Vec<String> {
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

pub fn split_list(value: &str, sep: char) -> Vec<String> {
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

/// Slug for `tickets/NN-<slug>.md` from a title.
pub fn title_slug(title: &str) -> String {
    title.to_lowercase().replace(' ', "-")
}

/// Next unused two-digit ticket id in `tickets`.
pub fn next_ticket_id(tickets: &[Ticket]) -> String {
    let max = tickets
        .iter()
        .filter_map(|t| t.id.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    format!("{:02}", max + 1)
}

fn format_blocked_by(ids: &[String]) -> String {
    if ids.is_empty() {
        "None (can start immediately)".to_string()
    } else {
        ids.join(", ")
    }
}

/// Markdown for a new ticket file.
pub fn render_ticket_markdown(
    id: &str,
    title: &str,
    what: &str,
    blocked_by: &[String],
    files: &[String],
    verify: &[String],
) -> String {
    let mut body = format!(
        "# {id}: {title}\n\n**What to build:** {what}\n\n**Blocked by:** {}\n\n**Status:** ready-for-agent\n\n",
        format_blocked_by(blocked_by)
    );
    if !files.is_empty() {
        body.push_str(&format!("**Files:** {}\n\n", files.join(", ")));
    }
    if !verify.is_empty() {
        body.push_str(&format!("**Verify:** {}\n\n", verify.join("; ")));
    }
    body.push_str("- [ ] done\n");
    body
}

/// Write a ticket file under `dir` and return the parsed ticket.
pub fn add_ticket(
    dir: &Path,
    id: Option<&str>,
    title: &str,
    what: &str,
    blocked_by: Vec<String>,
    files: Vec<String>,
    verify: Vec<String>,
) -> Result<Ticket> {
    let title = title.trim();
    if title.is_empty() {
        bail!("title must not be empty");
    }
    let existing = load_tickets(dir)?;
    for b in &blocked_by {
        if !existing.iter().any(|t| &t.id == b) {
            bail!("blocked by unknown ticket {b}");
        }
    }
    let id = match id {
        Some(raw) => normalize_id(raw).context("invalid ticket id")?,
        None => next_ticket_id(&existing),
    };
    if existing.iter().any(|t| t.id == id) {
        bail!("ticket {id} already exists");
    }
    let slug = title_slug(title);
    if slug.is_empty() {
        bail!("title must contain alphanumeric characters");
    }
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let path = dir.join(format!("{id}-{slug}.md"));
    if path.exists() {
        bail!("ticket file `{}` already exists", path.display());
    }
    let md = render_ticket_markdown(&id, title, what, &blocked_by, &files, &verify);
    std::fs::write(&path, &md).with_context(|| format!("writing {}", path.display()))?;
    parse_ticket(&md, &path)
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

/// Ids of the ready tickets `dispatch` would accept now: a pair is free (neither busy nor
/// dirty), and the ticket's `files` overlap no ticket in progress (unless the run allows overlap).
pub fn dispatchable(tickets: &[Ticket], state: &State, dirty: &[u32]) -> Vec<String> {
    let busy_pairs: Vec<u32> = state
        .tickets
        .values()
        .filter(|ts| matches!(ts.status, TicketStatus::Doing | TicketStatus::Review))
        .filter_map(|ts| ts.pair)
        .collect();
    let free_pair = (1..=state.run.pairs).any(|n| !busy_pairs.contains(&n) && !dirty.contains(&n));
    let in_progress: Vec<&Ticket> = tickets
        .iter()
        .filter(|t| {
            state.tickets.get(&t.id).is_some_and(|ts| {
                matches!(
                    ts.status,
                    TicketStatus::Doing | TicketStatus::Review | TicketStatus::AwaitingApproval
                )
            })
        })
        .collect();
    ready(tickets, state)
        .into_iter()
        .filter(|t| {
            free_pair
                && (state.run.allows_overlap()
                    || in_progress.iter().all(|other| !files_overlap(t, other)))
        })
        .map(|t| t.id.clone())
        .collect()
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

/// True when `path` could be touched by `t`'s `files` globs. Tickets with no
/// `files` cover every path.
pub fn files_cover(t: &Ticket, path: &str) -> bool {
    t.files.is_empty() || t.files.iter().any(|f| globs_overlap(f, path))
}

/// Shape of a work graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Width {
    /// Tickets on the longest blocker chain.
    pub longest_chain: usize,
    /// Most tickets ready at once while tickets land one by one in id order.
    pub max_parallel: usize,
    /// `max_parallel` with tickets that share a `files` path counted as one slot: what
    /// `--overlap serial` dispatch can run at once.
    pub max_parallel_serial: usize,
}

/// Depth of `id` in blocker chains: its own ticket plus its deepest blocker.
/// Unknown ids and cycles count as depth 0, so they never extend a chain.
fn chain_depth<'a>(
    id: &'a str,
    by_id: &BTreeMap<&'a str, &'a Ticket>,
    memo: &mut BTreeMap<&'a str, usize>,
    visiting: &mut BTreeSet<&'a str>,
) -> usize {
    if let Some(&d) = memo.get(id) {
        return d;
    }
    let Some(&t) = by_id.get(id) else { return 0 };
    if !visiting.insert(id) {
        return 0;
    }
    let deepest = t
        .blocked_by
        .iter()
        .map(|b| chain_depth(b.as_str(), by_id, memo, visiting))
        .max()
        .unwrap_or(0);
    visiting.remove(id);
    memo.insert(id, deepest + 1);
    deepest + 1
}

/// True when both tickets list `Files:` and those globs could touch the same path. Unlike
/// `files_overlap`, a ticket with no `Files:` shares no path with anything.
fn shares_path(a: &Ticket, b: &Ticket) -> bool {
    !a.files.is_empty() && !b.files.is_empty() && files_overlap(a, b)
}

/// Most tickets from `ready` (in order) that pairwise share no `Files:` path, picked greedily.
fn disjoint_count(ready: &[&Ticket]) -> usize {
    let mut picked: Vec<&Ticket> = Vec::new();
    for t in ready {
        if picked.iter().all(|p| !shares_path(t, p)) {
            picked.push(t);
        }
    }
    picked.len()
}

/// Longest blocker chain, and the most tickets ready at once when tickets land
/// one by one (the lowest ready id first), with and without `files` overlap counted.
/// Duplicate ids count once.
pub fn width(tickets: &[Ticket]) -> Width {
    let mut by_id: BTreeMap<&str, &Ticket> = BTreeMap::new();
    for t in tickets {
        by_id.entry(t.id.as_str()).or_insert(t);
    }
    let mut memo = BTreeMap::new();
    let longest_chain = by_id
        .keys()
        .map(|id| chain_depth(id, &by_id, &mut memo, &mut BTreeSet::new()))
        .max()
        .unwrap_or(0);

    let mut landed: BTreeSet<&str> = BTreeSet::new();
    let mut max_parallel = 0;
    let mut max_parallel_serial = 0;
    loop {
        let mut ready: Vec<&Ticket> = by_id
            .values()
            .copied()
            .filter(|t| {
                !landed.contains(t.id.as_str())
                    && t.blocked_by.iter().all(|b| landed.contains(b.as_str()))
            })
            .collect();
        ready.sort_by(|a, b| id_cmp(&a.id, &b.id));
        max_parallel = max_parallel.max(ready.len());
        max_parallel_serial = max_parallel_serial.max(disjoint_count(&ready));
        let Some(&next) = ready.first() else { break };
        landed.insert(next.id.as_str());
    }
    Width {
        longest_chain,
        max_parallel,
        max_parallel_serial,
    }
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
    fn width_of_a_chain_is_serial() {
        let ts = [
            t("01", &[], &[]),
            t("02", &["01"], &[]),
            t("03", &["02"], &[]),
        ];
        assert_eq!(
            width(&ts),
            Width {
                longest_chain: 3,
                max_parallel: 1,
                max_parallel_serial: 1,
            }
        );
    }

    #[test]
    fn width_of_a_tracer_with_fan_out() {
        let mut ts = vec![t("01", &[], &[])];
        ts.extend((2..=7).map(|i| t(&format!("{i:02}"), &["01"], &[])));
        assert_eq!(
            width(&ts),
            Width {
                longest_chain: 2,
                max_parallel: 6,
                max_parallel_serial: 6,
            }
        );
    }

    #[test]
    fn width_never_readies_unknown_blockers_or_cycles() {
        let unknown = [t("01", &["09"], &[])];
        assert_eq!(
            width(&unknown),
            Width {
                longest_chain: 1,
                max_parallel: 0,
                max_parallel_serial: 0,
            }
        );
        let cycle = [t("02", &["03"], &[]), t("03", &["02"], &[])];
        assert_eq!(width(&cycle).max_parallel, 0);
    }

    #[test]
    fn serial_width_counts_shared_files_as_one_slot() {
        let shared = [
            t("01", &[], &["src/a.rs"]),
            t("02", &[], &["src/a.rs"]),
            t("03", &[], &["src/a.rs"]),
            t("04", &[], &["src/a.rs"]),
        ];
        assert_eq!(
            width(&shared),
            Width {
                longest_chain: 1,
                max_parallel: 4,
                max_parallel_serial: 1,
            }
        );
        let unlisted = [t("01", &[], &[]), t("02", &[], &[]), t("03", &[], &[])];
        assert_eq!(width(&unlisted).max_parallel_serial, 3);
        let mixed = [
            t("01", &[], &["src/a.rs"]),
            t("02", &[], &["src/b.rs"]),
            t("03", &[], &["src/a.rs"]),
        ];
        assert_eq!(width(&mixed).max_parallel_serial, 2);
    }

    #[test]
    fn overlap_allow_lets_shared_file_tickets_dispatch_together() {
        let ts = [t("01", &[], &["src/a.rs"]), t("02", &[], &["src/a.rs"])];
        let mut st = State::new("demo");
        st.run.pairs = 2;
        st.tickets.insert(
            "01".into(),
            crate::state::TicketState {
                status: TicketStatus::Doing,
                pair: Some(1),
                ..Default::default()
            },
        );
        assert_eq!(dispatchable(&ts, &st, &[]), Vec::<String>::new());
        st.run.overlap = "allow".into();
        assert_eq!(dispatchable(&ts, &st, &[]), vec!["02".to_string()]);
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

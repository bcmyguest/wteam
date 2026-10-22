//! The stop report: done tickets, blocked tickets with reasons and recommended
//! actions, and follow-up patterns that reached the rule of three.

use anyhow::Result;
use serde_json::{Value, json};

use crate::paths::Paths;
use crate::state::TicketStatus;
use crate::work;

/// Occurrences at which a follow-up key becomes a ticket.
const PROMOTE_AT: usize = 3;

/// Follow-up pattern keys in `queue.md` with their occurrence counts, most frequent first.
pub fn follow_up_counts(paths: &Paths) -> Result<Vec<(String, usize)>> {
    let text = match std::fs::read_to_string(paths.queue_file()) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let mut counts: Vec<(String, usize)> = Vec::new();
    for key in text.lines().filter_map(queue_key) {
        match counts.iter_mut().find(|(k, _)| k == key) {
            Some((_, n)) => *n += 1,
            None => counts.push((key.to_string(), 1)),
        }
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    Ok(counts)
}

/// The `key` of a `- [key] NN: text` queue line.
fn queue_key(line: &str) -> Option<&str> {
    let rest = line.trim().strip_prefix("- [")?;
    let key = rest[..rest.find(']')?].trim();
    (!key.is_empty()).then_some(key)
}

/// What to do about a blocked ticket, from the reason it carries.
fn recommended_action(reason: &str, paths: &Paths) -> String {
    if reason.starts_with("review failed") {
        return "split the ticket or raise the worker model".to_string();
    }
    if reason.contains(" turn failed: ") {
        let log = reason
            .split_once("(log: ")
            .and_then(|(_, r)| r.split_once(')'))
            .map(|(p, _)| p.to_string())
            .unwrap_or_else(|| paths.logs_dir().display().to_string());
        return format!("check the CLI log at {log}");
    }
    format!("re-plan: {reason}")
}

/// The stop report as JSON: `done`, `blocked`, `follow_ups` and `next_step`.
pub fn report_data(paths: &Paths) -> Result<Value> {
    let tickets = work::load_tickets(&paths.tickets_dir())?;
    let state = crate::state::load(paths)?;
    let dispatch = state.run.dispatch.clone();
    let title = |id: &str| {
        tickets
            .iter()
            .find(|t| t.id == id)
            .map_or_else(String::new, |t| t.title.clone())
    };
    let mut done = Vec::new();
    let mut blocked = Vec::new();
    for (id, ts) in &state.tickets {
        match ts.status {
            TicketStatus::Done => done.push(json!({
                "id": id, "title": title(id), "commit": ts.commit,
            })),
            TicketStatus::Blocked => {
                let reason = ts.blocked_reason.clone().unwrap_or_default();
                blocked.push(json!({
                    "id": id,
                    "title": title(id),
                    "reason": reason,
                    "action": recommended_action(&reason, paths),
                    "replans": ts.replans,
                }));
            }
            _ => {}
        }
    }
    let dirty = crate::worktree::dirty_pairs(paths, state.run.pairs);
    let (mut promote, mut watching) = (Vec::new(), Vec::new());
    for (key, count) in follow_up_counts(paths)? {
        let row = json!({ "key": key, "count": count });
        if count >= PROMOTE_AT {
            promote.push(row);
        } else {
            watching.push(row);
        }
    }
    let next_step = if !work::ready(&tickets, &state).is_empty() {
        "`wteam run`"
    } else if !blocked.is_empty() {
        "re-plan the blocked tickets, then `wteam pickup`"
    } else if tickets.iter().all(|t| {
        state
            .tickets
            .get(&t.id)
            .is_some_and(|s| s.status == TicketStatus::Done)
    }) {
        "nothing left to run; every ticket is done"
    } else {
        "`wteam wait` for the tickets still in progress"
    };
    Ok(json!({
        "done": done,
        "blocked": blocked,
        "follow_ups": { "promote": promote, "watching": watching },
        "dirty_worktrees": dirty,
        "dispatch": dispatch,
        "next_step": next_step,
    }))
}

/// Render the stop report as markdown.
pub fn stop_report(paths: &Paths) -> Result<String> {
    let data = report_data(paths)?;
    let s = |v: &Value, k: &str| v[k].as_str().unwrap_or_default().to_string();
    let rows = |v: &Value| v.as_array().cloned().unwrap_or_default();
    let mut out = format!("# Stop report: {}\n\n## Done\n", paths.slug);
    let done = rows(&data["done"]);
    if done.is_empty() {
        out.push_str("none\n");
    }
    for d in &done {
        let sha = d["commit"]
            .as_str()
            .map_or(String::new(), |c| format!(" ({})", &c[..c.len().min(7)]));
        out.push_str(&format!("- {}: {}{sha}\n", s(d, "id"), s(d, "title")));
    }
    out.push_str("\n## Blocked\n");
    let blocked = rows(&data["blocked"]);
    if blocked.is_empty() {
        out.push_str("none\n");
    }
    for b in &blocked {
        out.push_str(&format!(
            "- {}: {}\n  reason: {}\n  action: {}\n",
            s(b, "id"),
            s(b, "title"),
            s(b, "reason"),
            s(b, "action")
        ));
        if b["replans"].as_u64().unwrap_or(0) > 0 {
            out.push_str("  replan: re-planned once; the planner's re-plan did not unblock it\n");
        }
    }
    out.push_str(&format!(
        "Dispatch: {}\n",
        data["dispatch"].as_str().unwrap_or_default()
    ));
    let dirty: Vec<String> = rows(&data["dirty_worktrees"])
        .iter()
        .filter_map(Value::as_u64)
        .map(|n| format!("pair {n}"))
        .collect();
    if dirty.is_empty() {
        out.push_str("Dirty worktrees: none\n");
    } else {
        out.push_str(&format!("Dirty worktrees: {}\n", dirty.join(", ")));
    }
    out.push_str("\n## Follow-ups\n");
    let (promote, watching) = (
        rows(&data["follow_ups"]["promote"]),
        rows(&data["follow_ups"]["watching"]),
    );
    if promote.is_empty() && watching.is_empty() {
        out.push_str("none\n");
    }
    for p in &promote {
        out.push_str(&format!(
            "- [{}] x{}: promote to a ticket\n",
            s(p, "key"),
            p["count"]
        ));
    }
    for w in &watching {
        out.push_str(&format!("- [{}] x{}: watching\n", s(w, "key"), w["count"]));
    }
    out.push_str(&format!("\n**Next step:** {}\n", s(&data, "next_step")));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_keys_across_tickets_and_ignores_other_lines() {
        let tmp = tempfile::tempdir().unwrap();
        let p = Paths::new(tmp.path(), "s");
        std::fs::create_dir_all(p.effort_dir()).unwrap();
        std::fs::write(
            p.queue_file(),
            "- [dup-parse] 01: a (from reviewer.1)\nnoise\n- [other] 02: b (from reviewer.1)\n- [dup-parse] 03: c (from reviewer.2)\n- [] 04: x\n",
        )
        .unwrap();
        assert_eq!(
            follow_up_counts(&p).unwrap(),
            vec![("dup-parse".to_string(), 2), ("other".to_string(), 1)]
        );
    }

    #[test]
    fn missing_queue_has_no_counts() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(
            follow_up_counts(&Paths::new(tmp.path(), "s"))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn actions_follow_the_block_reason() {
        let p = Paths::new("/r", "s");
        assert_eq!(
            recommended_action("review failed 3 rounds: x", &p),
            "split the ticket or raise the worker model"
        );
        assert_eq!(
            recommended_action(
                "worker.1 turn failed: claude exited (log: /l/w-1.log): boom",
                &p
            ),
            "check the CLI log at /l/w-1.log"
        );
        assert_eq!(recommended_action("no api", &p), "re-plan: no api");
    }
}

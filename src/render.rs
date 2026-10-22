//! Mermaid rendering of the team graph and the work graph.

use crate::state::{State, TicketStatus};
use crate::team::{NodeKind, Team};
use crate::work::Ticket;

const STATUSES: [(TicketStatus, &str, &str); 6] = [
    (TicketStatus::Todo, "todo", "fill:#eee,stroke:#999"),
    (TicketStatus::Doing, "doing", "fill:#cde,stroke:#46a"),
    (TicketStatus::Review, "review", "fill:#fec,stroke:#c82"),
    (
        TicketStatus::AwaitingApproval,
        "awaiting_approval",
        "fill:#fdf,stroke:#a4a",
    ),
    (TicketStatus::Done, "done", "fill:#cec,stroke:#383"),
    (TicketStatus::Blocked, "blocked", "fill:#fcc,stroke:#c33"),
];

/// Collision-free Mermaid node id: alphanumerics pass through, `_` doubles, every
/// other char becomes `_u<hex>_`. The word `end` (any case) is reserved, so it gets an
/// `n_` prefix, which no escaped id can produce.
fn mm_id(id: &str) -> String {
    let mut out = String::new();
    for c in id.chars() {
        if c.is_alphanumeric() {
            out.push(c);
        } else if c == '_' {
            out.push_str("__");
        } else {
            out.push_str(&format!("_u{:x}_", c as u32));
        }
    }
    if out.eq_ignore_ascii_case("end") {
        out.insert_str(0, "n_");
    }
    out
}

/// Quote text for a Mermaid label.
fn mm_text(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "#quot;"))
}

/// Mermaid flowchart of a team: `[ ]` agent, `[/ /]` command, `{{ }}` human.
pub fn team_mermaid(team: &Team) -> String {
    let mut out = String::from("flowchart LR\n");
    for role in team.roles.values() {
        let name = role.label.as_deref().unwrap_or(&role.name);
        let id = mm_id(&role.name);
        let line = match &role.kind {
            NodeKind::Agent { count, .. } if *count > 1 => {
                format!("{id}[{}]", mm_text(&format!("{name} x{count}")))
            }
            NodeKind::Agent { .. } => format!("{id}[{}]", mm_text(name)),
            NodeKind::Command { .. } => format!("{id}[/{}/]", mm_text(name)),
            NodeKind::Human => format!("{id}{{{{{}}}}}", mm_text(name)),
        };
        out.push_str(&format!("  {line}\n"));
    }
    for l in &team.links {
        let on = l.on.map_or("any", |e| e.name());
        let kind = l.kind.name();
        let text = match &l.label {
            Some(label) => format!("{label} {on}/{kind}"),
            None => format!("{on}/{kind}"),
        };
        out.push_str(&format!(
            "  {} -->|{}| {}\n",
            mm_id(&l.from),
            mm_text(&text),
            mm_id(&l.to)
        ));
    }
    out
}

/// `flowchart TD`: a node per ticket, an edge blocker→ticket, a class per status.
pub fn work_mermaid(tickets: &[Ticket], state: &State) -> String {
    let mut out = String::from("flowchart TD\n");
    for t in tickets {
        let status = state
            .tickets
            .get(&t.id)
            .map(|s| s.status)
            .unwrap_or_default();
        let class = STATUSES
            .iter()
            .find(|(s, _, _)| *s == status)
            .map_or("todo", |(_, n, _)| n);
        let title = t.title.replace('"', "'");
        out.push_str(&format!(
            "  t{}[\"{}: {}\"]:::{}\n",
            t.id, t.id, title, class
        ));
    }
    for t in tickets {
        for b in &t.blocked_by {
            if tickets.iter().any(|x| &x.id == b) {
                out.push_str(&format!("  t{b} --> t{}\n", t.id));
            }
        }
    }
    for (_, name, style) in STATUSES {
        out.push_str(&format!("  classDef {name} {style}\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn t(id: &str, title: &str, blocked: &[&str]) -> Ticket {
        Ticket {
            id: id.into(),
            title: title.into(),
            what: String::new(),
            blocked_by: blocked.iter().map(|s| s.to_string()).collect(),
            status_label: String::new(),
            files: vec![],
            verify: vec![],
            criteria: vec![],
            path: PathBuf::new(),
            body: String::new(),
        }
    }

    #[test]
    fn mermaid_ids_are_collision_free_and_avoid_end() {
        assert_ne!(mm_id("a-b"), mm_id("a_b"));
        assert_ne!(mm_id("a-b"), mm_id("a b"));
        assert_ne!(mm_id("a_b"), mm_id("a__b"));
        assert_ne!(mm_id("a_u2d_b"), mm_id("a-b"));
        for reserved in ["end", "End", "END"] {
            let id = mm_id(reserved);
            assert!(!id.eq_ignore_ascii_case("end"), "{id}");
        }
        assert_ne!(mm_id("end"), mm_id("n_end"));
        assert_eq!(mm_id("worker"), "worker");
    }

    #[test]
    fn team_graph_renders_colliding_and_reserved_names() {
        let src = "digraph t { a1 [label=x]; \"a-b\"; a_b; end; \"a-b\" -> a_b; a_b -> end }";
        let team = Team::from_graph(&crate::team::dot::parse(src).unwrap(), 1).unwrap();
        let m = team_mermaid(&team);
        assert!(m.contains(&format!("{}[\"a-b\"]", mm_id("a-b"))), "{m}");
        assert!(m.contains(&format!(
            "{} -->|\"any/pair\"| {}",
            mm_id("a_b"),
            mm_id("end")
        )));
        assert!(!m.contains(" end\n") && !m.contains("  end["), "{m}");
    }

    #[test]
    fn work_graph_has_nodes_edges_and_status_classes() {
        let ts = [t("01", "First", &[]), t("02", "Se\"cond", &["01", "99"])];
        let mut s = State::new("s");
        s.ticket_mut("01").status = TicketStatus::Done;
        let m = work_mermaid(&ts, &s);
        assert!(m.starts_with("flowchart TD\n"));
        assert!(m.contains("t01[\"01: First\"]:::done"));
        assert!(m.contains("t02[\"02: Se'cond\"]:::todo"));
        assert!(m.contains("t01 --> t02"));
        assert!(!m.contains("t99"));
        assert!(m.contains("classDef blocked"));
    }
}

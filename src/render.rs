//! Mermaid rendering of the team graph and the work graph.

use crate::state::{State, TicketStatus};
use crate::team::Team;
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

pub fn team_mermaid(team: &Team) -> String {
    let _ = team;
    String::from("flowchart LR\n")
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

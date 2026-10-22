//! The team graph: which roles exist, how many instances, and who routes to whom.

pub mod agents;
pub mod dot;

use std::collections::BTreeMap;

use anyhow::{Result, bail};
use serde::Serialize;

/// Events agents emit on a ticket; edges route on them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum TeamEvent {
    /// Orchestrator assigns a ticket.
    Dispatch,
    /// Worker finished a round.
    Submit,
    Pass,
    Fail,
    Blocked,
    /// Free-form message.
    Message,
}

impl TeamEvent {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "dispatch" => Self::Dispatch,
            "submit" => Self::Submit,
            "pass" => Self::Pass,
            "fail" => Self::Fail,
            "blocked" => Self::Blocked,
            "message" => Self::Message,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkKind {
    /// Instance i talks to instance i.
    Pair,
    /// Every instance talks to instance 1.
    FanIn,
    /// One sender reaches every instance.
    Broadcast,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NodeKind {
    /// `shape=box` (default): an agent role.
    Agent {
        /// Agent definition name (TOML).
        agent: String,
        count: u32,
        /// Permission profile override.
        profile: Option<String>,
        /// Role prompt override (`@file.md` or inline).
        prompt: Option<String>,
    },
    /// `shape=parallelogram`: a shell check; non-zero exit is a finding.
    Command { script: String },
    /// `shape=hexagon`: a human approval gate.
    Human,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Role {
    pub name: String,
    pub kind: NodeKind,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Link {
    pub from: String,
    pub to: String,
    /// Event the link routes; `None` routes every event.
    pub on: Option<TeamEvent>,
    pub kind: LinkKind,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Team {
    pub name: String,
    pub roles: BTreeMap<String, Role>,
    pub links: Vec<Link>,
    /// Review-round cap per ticket.
    pub max_visits: u32,
    /// Orchestrator context size that triggers a handoff.
    pub handoff_tokens: u64,
}

impl Team {
    /// The built-in default topology without parsing DOT:
    /// planner → orchestrator → N×(worker ↔ reviewer).
    pub fn default_topology(pairs: u32) -> Team {
        let agent = |name: &str, count: u32| Role {
            name: name.into(),
            kind: NodeKind::Agent {
                agent: name.into(),
                count,
                profile: None,
                prompt: None,
            },
            label: None,
        };
        let link = |from: &str, to: &str, on: TeamEvent, kind: LinkKind| Link {
            from: from.into(),
            to: to.into(),
            on: Some(on),
            kind,
            label: None,
        };
        let mut roles = BTreeMap::new();
        for r in [
            agent("planner", 1),
            agent("orchestrator", 1),
            agent("worker", pairs),
            agent("reviewer", pairs),
        ] {
            roles.insert(r.name.clone(), r);
        }
        Team {
            name: "default".into(),
            roles,
            links: vec![
                link(
                    "orchestrator",
                    "worker",
                    TeamEvent::Dispatch,
                    LinkKind::Broadcast,
                ),
                link("worker", "reviewer", TeamEvent::Submit, LinkKind::Pair),
                link("reviewer", "worker", TeamEvent::Fail, LinkKind::Pair),
                link("reviewer", "orchestrator", TeamEvent::Pass, LinkKind::FanIn),
                link(
                    "worker",
                    "orchestrator",
                    TeamEvent::Blocked,
                    LinkKind::FanIn,
                ),
            ],
            max_visits: 3,
            handoff_tokens: 150_000,
        }
    }

    /// Build a team from a parsed DOT graph, substituting `$pairs` in `count`.
    pub fn from_graph(graph: &dot::Graph, pairs: u32) -> Result<Team> {
        let _ = (graph, pairs);
        bail!("not implemented: Team::from_graph")
    }

    /// Outgoing links from `role` that route `event`.
    pub fn routes(&self, role: &str, event: TeamEvent) -> Vec<&Link> {
        self.links
            .iter()
            .filter(|l| l.from == role && l.on.is_none_or(|e| e == event))
            .collect()
    }

    /// Instance count of a role (1 for non-agent nodes).
    pub fn count(&self, role: &str) -> u32 {
        match self.roles.get(role).map(|r| &r.kind) {
            Some(NodeKind::Agent { count, .. }) => *count,
            Some(_) => 1,
            None => 0,
        }
    }

    /// Target instance indices for a link fired by instance `from_index`.
    pub fn targets(&self, link: &Link, from_index: u32) -> Vec<u32> {
        let n = self.count(&link.to);
        match link.kind {
            LinkKind::Pair => vec![from_index.min(n.max(1))],
            LinkKind::FanIn => vec![1],
            LinkKind::Broadcast => (1..=n).collect(),
        }
    }
}

/// Instance id for a role and 1-based index, e.g. `worker.2`.
pub fn instance_id(role: &str, index: u32) -> String {
    format!("{role}.{index}")
}

/// Split `worker.2` into (`worker`, 2). A bare role means index 1.
pub fn parse_instance(id: &str) -> (String, u32) {
    match id.rsplit_once('.') {
        Some((role, idx)) => match idx.parse() {
            Ok(i) => (role.to_string(), i),
            Err(_) => (id.to_string(), 1),
        },
        None => (id.to_string(), 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_topology_routes_pairs() {
        let t = Team::default_topology(3);
        let submit = t.routes("worker", TeamEvent::Submit);
        assert_eq!(submit.len(), 1);
        assert_eq!(t.targets(submit[0], 2), vec![2]);
        let pass = t.routes("reviewer", TeamEvent::Pass);
        assert_eq!(t.targets(pass[0], 3), vec![1]);
    }

    #[test]
    fn instance_ids_round_trip() {
        assert_eq!(
            parse_instance(&instance_id("worker", 4)),
            ("worker".into(), 4)
        );
        assert_eq!(parse_instance("orchestrator"), ("orchestrator".into(), 1));
    }
}

//! The team graph: which roles exist, how many instances, and who routes to whom.

pub mod agents;
pub mod dot;

use std::collections::BTreeMap;

use anyhow::{Result, anyhow, bail};
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

    /// Lowercase name, as written in DOT `on=`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Dispatch => "dispatch",
            Self::Submit => "submit",
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Blocked => "blocked",
            Self::Message => "message",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkKind {
    /// Instance i talks to instance i.
    Pair,
    /// Every instance talks to instance 1.
    #[serde(rename = "fan-in")]
    FanIn,
    /// One sender reaches every instance.
    Broadcast,
}

impl LinkKind {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "pair" => Self::Pair,
            "fan-in" => Self::FanIn,
            "broadcast" => Self::Broadcast,
            _ => return None,
        })
    }

    /// Name as written in DOT `kind=`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Pair => "pair",
            Self::FanIn => "fan-in",
            Self::Broadcast => "broadcast",
        }
    }
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
        let num = |key: &str, default: u64| -> Result<u64> {
            match graph.attrs.get(key) {
                None => Ok(default),
                Some(v) => v
                    .parse()
                    .map_err(|_| anyhow!("graph attribute {key}={v:?} is not an integer")),
            }
        };
        let max_visits = u32::try_from(num("max_visits", 3)?)
            .map_err(|_| anyhow!("graph attribute max_visits is too large"))?;
        let handoff_tokens = num("handoff_tokens", 150_000)?;

        let mut roles = BTreeMap::new();
        for node in &graph.nodes {
            let a = &node.attrs;
            let get = |k: &str| a.get(k).cloned();
            let kind = match a.get("shape").map(String::as_str).unwrap_or("box") {
                "box" => {
                    let count = match a.get("count").map(String::as_str) {
                        None => 1,
                        Some("$pairs") => pairs,
                        Some(v) => v.parse().map_err(|_| {
                            anyhow!("node {}: count={v:?} is not an integer or $pairs", node.id)
                        })?,
                    };
                    NodeKind::Agent {
                        agent: get("agent").unwrap_or_else(|| node.id.clone()),
                        count,
                        profile: get("profile"),
                        prompt: get("prompt"),
                    }
                }
                "parallelogram" => NodeKind::Command {
                    script: get("script").ok_or_else(|| {
                        anyhow!("node {}: shape=parallelogram needs script=", node.id)
                    })?,
                },
                "hexagon" => NodeKind::Human,
                other => bail!(
                    "node {}: unknown shape {other:?} (box, parallelogram, hexagon)",
                    node.id
                ),
            };
            roles.insert(
                node.id.clone(),
                Role {
                    name: node.id.clone(),
                    kind,
                    label: get("label"),
                },
            );
        }

        let mut links = Vec::new();
        for e in &graph.edges {
            let on = match e.attrs.get("on") {
                None => None,
                Some(v) => Some(
                    TeamEvent::parse(v)
                        .ok_or_else(|| anyhow!("edge {} -> {}: unknown on={v:?}", e.from, e.to))?,
                ),
            };
            let kind = match e.attrs.get("kind") {
                None => LinkKind::Pair,
                Some(v) => LinkKind::parse(v)
                    .ok_or_else(|| anyhow!("edge {} -> {}: unknown kind={v:?}", e.from, e.to))?,
            };
            links.push(Link {
                from: e.from.clone(),
                to: e.to.clone(),
                on,
                kind,
                label: e.attrs.get("label").cloned(),
            });
        }

        Ok(Team {
            name: graph.name.clone(),
            roles,
            links,
            max_visits,
            handoff_tokens,
        })
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

    fn team(src: &str, pairs: u32) -> Result<Team> {
        Team::from_graph(&dot::parse(src).unwrap(), pairs)
    }

    #[test]
    fn default_preset_matches_default_topology() {
        let src = crate::config::preset("default").unwrap();
        let mut t = team(src, 3).unwrap();
        t.name = "default".into();
        assert_eq!(t, Team::default_topology(3));
    }

    #[test]
    fn every_preset_parses() {
        for name in crate::config::PRESETS {
            let t = team(crate::config::preset(name).unwrap(), 2).unwrap();
            assert_eq!(t.count("worker"), 2, "{name}");
        }
    }

    #[test]
    fn shapes_map_to_node_kinds() {
        let t = team(
            "digraph t { a; b [shape=parallelogram, script=\"make\"]; c [shape=hexagon]; d [agent=x, count=2, profile=full, prompt=hi, label=D] }",
            1,
        )
        .unwrap();
        assert_eq!(
            t.roles["a"].kind,
            NodeKind::Agent {
                agent: "a".into(),
                count: 1,
                profile: None,
                prompt: None
            }
        );
        assert_eq!(
            t.roles["b"].kind,
            NodeKind::Command {
                script: "make".into()
            }
        );
        assert_eq!(t.roles["c"].kind, NodeKind::Human);
        assert_eq!(t.count("d"), 2);
        assert_eq!(t.roles["d"].label.as_deref(), Some("D"));
    }

    #[test]
    fn edge_defaults_and_attrs() {
        let t = team(
            "digraph t { a -> b; b -> c [on=pass, kind=fan-in, label=ok] }",
            1,
        )
        .unwrap();
        assert_eq!((t.links[0].on, t.links[0].kind), (None, LinkKind::Pair));
        assert_eq!(t.links[1].on, Some(TeamEvent::Pass));
        assert_eq!(t.links[1].kind, LinkKind::FanIn);
        assert_eq!(t.links[1].label.as_deref(), Some("ok"));
        assert_eq!((t.max_visits, t.handoff_tokens), (3, 150_000));
    }

    #[test]
    fn link_kind_serializes_as_written_in_dot() {
        for k in [LinkKind::Pair, LinkKind::FanIn, LinkKind::Broadcast] {
            assert_eq!(serde_json::to_value(k).unwrap(), k.name());
            assert_eq!(LinkKind::parse(k.name()), Some(k));
        }
        assert_eq!(LinkKind::parse("fan_in"), None);
    }

    #[test]
    fn graph_limits_override_defaults() {
        let t = team(
            "digraph t { graph [max_visits=5, handoff_tokens=1000] a }",
            1,
        )
        .unwrap();
        assert_eq!((t.max_visits, t.handoff_tokens), (5, 1000));
    }

    #[test]
    fn rejects_bad_attributes() {
        for src in [
            "digraph t { a -> b [on=bogus] }",
            "digraph t { a -> b [kind=bogus] }",
            "digraph t { a [shape=parallelogram] }",
            "digraph t { a [shape=star] }",
            "digraph t { a [count=many] }",
            "digraph t { graph [max_visits=x] a }",
        ] {
            assert!(team(src, 1).is_err(), "{src}");
        }
    }

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

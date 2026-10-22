//! A Graphviz DOT subset (borrowed from Fabro/Attractor): one named `digraph`,
//! node statements, edge chains (`a -> b -> c`), attribute lists, `graph [..]`
//! and bare `key=value` graph attributes, `//`, `#` and `/* */` comments,
//! quoted strings with `\"` escapes. Semicolons optional.

use std::collections::BTreeMap;

use anyhow::{Result, bail};

pub type Attrs = BTreeMap<String, String>;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Node {
    pub id: String,
    pub attrs: Attrs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub attrs: Attrs,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Graph {
    pub name: String,
    pub attrs: Attrs,
    /// In first-mention order; nodes only mentioned in edges appear with empty attrs.
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

/// Parse DOT source. Errors carry a line number.
pub fn parse(src: &str) -> Result<Graph> {
    let _ = src;
    bail!("not implemented: dot::parse")
}

//! A Graphviz DOT subset (borrowed from Fabro/Attractor): one named `digraph`,
//! node statements, edge chains (`a -> b -> c`), attribute lists, `graph [..]`
//! and bare `key=value` graph attributes, `node [..]`/`edge [..]` defaults,
//! `//`, `#` and `/* */` comments, quoted strings with `\"` escapes.
//! Semicolons and commas are optional.

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

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok {
    /// Unquoted or quoted identifier; `quoted` blocks keyword treatment.
    Id {
        text: String,
        quoted: bool,
    },
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Eq,
    Semi,
    Comma,
    Arrow,
    Eof,
}

impl Tok {
    fn describe(&self) -> String {
        match self {
            Tok::Id { text, .. } => format!("`{text}`"),
            Tok::LBrace => "`{`".into(),
            Tok::RBrace => "`}`".into(),
            Tok::LBracket => "`[`".into(),
            Tok::RBracket => "`]`".into(),
            Tok::Eq => "`=`".into(),
            Tok::Semi => "`;`".into(),
            Tok::Comma => "`,`".into(),
            Tok::Arrow => "`->`".into(),
            Tok::Eof => "end of input".into(),
        }
    }
}

/// Split source into tokens, each with its 1-based line.
fn lex(src: &str) -> Result<Vec<(Tok, usize)>> {
    let chars: Vec<char> = src.chars().collect();
    let mut toks = Vec::new();
    let mut line = 1;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        match c {
            '\n' => {
                line += 1;
                i += 1;
            }
            c if c.is_whitespace() => i += 1,
            '#' => skip_line(&chars, &mut i),
            '/' if next == Some('/') => skip_line(&chars, &mut i),
            '/' if next == Some('*') => {
                let start = line;
                i += 2;
                loop {
                    match (chars.get(i), chars.get(i + 1)) {
                        (Some('*'), Some('/')) => {
                            i += 2;
                            break;
                        }
                        (Some(c), _) => {
                            if *c == '\n' {
                                line += 1;
                            }
                            i += 1;
                        }
                        (None, _) => bail!("line {start}: unterminated /* comment"),
                    }
                }
            }
            '{' | '}' | '[' | ']' | '=' | ';' | ',' => {
                toks.push((
                    match c {
                        '{' => Tok::LBrace,
                        '}' => Tok::RBrace,
                        '[' => Tok::LBracket,
                        ']' => Tok::RBracket,
                        '=' => Tok::Eq,
                        ';' => Tok::Semi,
                        _ => Tok::Comma,
                    },
                    line,
                ));
                i += 1;
            }
            '-' if next == Some('>') => {
                toks.push((Tok::Arrow, line));
                i += 2;
            }
            '"' => {
                let start = line;
                let mut text = String::new();
                i += 1;
                loop {
                    match chars.get(i) {
                        None => bail!("line {start}: unterminated string"),
                        Some('"') => {
                            i += 1;
                            break;
                        }
                        Some('\\') => {
                            match chars.get(i + 1) {
                                Some('"') => text.push('"'),
                                Some('\\') => text.push('\\'),
                                Some('n') => text.push('\n'),
                                Some('t') => text.push('\t'),
                                Some(o) => {
                                    text.push('\\');
                                    text.push(*o);
                                }
                                None => bail!("line {start}: unterminated string"),
                            }
                            if chars[i + 1] == '\n' {
                                line += 1;
                            }
                            i += 2;
                        }
                        Some(c) => {
                            if *c == '\n' {
                                line += 1;
                            }
                            text.push(*c);
                            i += 1;
                        }
                    }
                }
                toks.push((Tok::Id { text, quoted: true }, start));
            }
            c if is_id_char(c) => {
                let mut text = String::new();
                while let Some(&c) = chars.get(i) {
                    let arrow = c == '-' && chars.get(i + 1) == Some(&'>');
                    if !is_id_char(c) || arrow {
                        break;
                    }
                    text.push(c);
                    i += 1;
                }
                toks.push((
                    Tok::Id {
                        text,
                        quoted: false,
                    },
                    line,
                ));
            }
            other => bail!("line {line}: unexpected character `{other}`"),
        }
    }
    toks.push((Tok::Eof, line));
    Ok(toks)
}

fn skip_line(chars: &[char], i: &mut usize) {
    while *i < chars.len() && chars[*i] != '\n' {
        *i += 1;
    }
}

fn is_id_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '.' | '-' | '$')
}

struct Parser {
    toks: Vec<(Tok, usize)>,
    pos: usize,
    graph: Graph,
    node_defaults: Attrs,
    edge_defaults: Attrs,
}

impl Parser {
    fn peek(&self) -> &Tok {
        &self.toks[self.pos.min(self.toks.len() - 1)].0
    }

    fn line(&self) -> usize {
        self.toks[self.pos.min(self.toks.len() - 1)].1
    }

    fn bump(&mut self) -> Tok {
        let t = self.peek().clone();
        self.pos += 1;
        t
    }

    fn expect(&mut self, want: Tok) -> Result<()> {
        let line = self.line();
        let got = self.bump();
        if got != want {
            bail!(
                "line {line}: expected {}, found {}",
                want.describe(),
                got.describe()
            );
        }
        Ok(())
    }

    fn id(&mut self, what: &str) -> Result<String> {
        let line = self.line();
        match self.bump() {
            Tok::Id { text, .. } => Ok(text),
            other => bail!("line {line}: expected {what}, found {}", other.describe()),
        }
    }

    fn header(&mut self) -> Result<()> {
        let line = self.line();
        match self.bump() {
            Tok::Id {
                text,
                quoted: false,
            } if text == "digraph" => {}
            Tok::Id {
                text,
                quoted: false,
            } if text == "strict" => bail!("line {line}: `strict` graphs are not supported"),
            Tok::Id {
                text,
                quoted: false,
            } if text == "graph" => {
                bail!("line {line}: undirected `graph` is not supported; use `digraph`")
            }
            other => bail!(
                "line {line}: expected `digraph`, found {}",
                other.describe()
            ),
        }
        if matches!(self.peek(), Tok::LBrace) {
            bail!("line {}: digraph needs a name", self.line());
        }
        self.graph.name = self.id("digraph name")?;
        self.expect(Tok::LBrace)
    }

    fn attr_list(&mut self) -> Result<Attrs> {
        let mut attrs = Attrs::new();
        while matches!(self.peek(), Tok::LBracket) {
            self.bump();
            loop {
                match self.peek() {
                    Tok::RBracket => {
                        self.bump();
                        break;
                    }
                    Tok::Semi | Tok::Comma => {
                        self.bump();
                    }
                    _ => {
                        let k = self.id("attribute name")?;
                        self.expect(Tok::Eq)?;
                        let v = self.id("attribute value")?;
                        attrs.insert(k, v);
                    }
                }
            }
        }
        Ok(attrs)
    }

    /// Index of node `id`, creating it with the current node defaults.
    fn touch_node(&mut self, id: &str) -> usize {
        if let Some(i) = self.graph.nodes.iter().position(|n| n.id == id) {
            return i;
        }
        self.graph.nodes.push(Node {
            id: id.to_string(),
            attrs: self.node_defaults.clone(),
        });
        self.graph.nodes.len() - 1
    }

    fn statement(&mut self) -> Result<()> {
        let line = self.line();
        let (first, quoted) = match self.bump() {
            Tok::Id { text, quoted } => (text, quoted),
            Tok::LBrace => bail!("line {line}: subgraphs are not supported"),
            other => bail!("line {line}: unexpected {}", other.describe()),
        };
        if !quoted && matches!(self.peek(), Tok::LBracket) {
            match first.as_str() {
                "graph" => {
                    let a = self.attr_list()?;
                    self.graph.attrs.extend(a);
                    return Ok(());
                }
                "node" => {
                    let a = self.attr_list()?;
                    self.node_defaults.extend(a);
                    return Ok(());
                }
                "edge" => {
                    let a = self.attr_list()?;
                    self.edge_defaults.extend(a);
                    return Ok(());
                }
                _ => {}
            }
        }
        if matches!(self.peek(), Tok::Eq) {
            self.bump();
            let v = self.id("attribute value")?;
            self.graph.attrs.insert(first, v);
            return Ok(());
        }
        let mut chain = vec![first];
        while matches!(self.peek(), Tok::Arrow) {
            self.bump();
            chain.push(self.id("node id after `->`")?);
        }
        let attrs = self.attr_list()?;
        for id in &chain {
            self.touch_node(id);
        }
        if chain.len() == 1 {
            let i = self.touch_node(&chain[0]);
            self.graph.nodes[i].attrs.extend(attrs);
        } else {
            for pair in chain.windows(2) {
                let mut a = self.edge_defaults.clone();
                a.extend(attrs.clone());
                self.graph.edges.push(Edge {
                    from: pair[0].clone(),
                    to: pair[1].clone(),
                    attrs: a,
                });
            }
        }
        Ok(())
    }

    fn body(&mut self) -> Result<()> {
        loop {
            match self.peek() {
                Tok::RBrace => {
                    self.bump();
                    break;
                }
                Tok::Semi | Tok::Comma => {
                    self.bump();
                }
                Tok::Eof => bail!("line {}: missing closing `}}`", self.line()),
                _ => self.statement()?,
            }
        }
        if !matches!(self.peek(), Tok::Eof) {
            bail!(
                "line {}: unexpected {} after the graph",
                self.line(),
                self.peek().describe()
            );
        }
        Ok(())
    }
}

/// Parse DOT source. Errors carry a line number.
pub fn parse(src: &str) -> Result<Graph> {
    let mut p = Parser {
        toks: lex(src)?,
        pos: 0,
        graph: Graph::default(),
        node_defaults: Attrs::new(),
        edge_defaults: Attrs::new(),
    };
    p.header()?;
    p.body()?;
    Ok(p.graph)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err(src: &str) -> String {
        parse(src).unwrap_err().to_string()
    }

    #[test]
    fn parses_nodes_edges_and_graph_attrs() {
        let g = parse(
            "digraph t {\n graph [max_visits=5, x=\"a b\"]\n y=2\n a [shape=box]; b\n a -> b -> c [on=pass]\n}",
        )
        .unwrap();
        assert_eq!(g.name, "t");
        assert_eq!(g.attrs["max_visits"], "5");
        assert_eq!(g.attrs["x"], "a b");
        assert_eq!(g.attrs["y"], "2");
        let ids: Vec<_> = g.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["a", "b", "c"]);
        assert_eq!(g.nodes[0].attrs["shape"], "box");
        assert_eq!(g.edges.len(), 2);
        assert!(g.edges.iter().all(|e| e.attrs["on"] == "pass"));
        assert_eq!(
            (g.edges[1].from.as_str(), g.edges[1].to.as_str()),
            ("b", "c")
        );
    }

    #[test]
    fn applies_node_and_edge_defaults() {
        let g = parse("digraph t { node [shape=hexagon] edge [kind=fan-in] a -> b [on=pass] c }")
            .unwrap();
        assert_eq!(g.nodes[0].attrs["shape"], "hexagon");
        assert_eq!(g.nodes[2].attrs["shape"], "hexagon");
        assert_eq!(g.edges[0].attrs["kind"], "fan-in");
        assert_eq!(g.edges[0].attrs["on"], "pass");
    }

    #[test]
    fn handles_comments_and_escapes() {
        let g = parse(
            "// lead\ndigraph t { # note\n /* multi\nline */ a [label=\"say \\\"hi\\\"\", n=3.5]\n}",
        )
        .unwrap();
        assert_eq!(g.nodes[0].attrs["label"], "say \"hi\"");
        assert_eq!(g.nodes[0].attrs["n"], "3.5");
    }

    #[test]
    fn arrow_without_spaces() {
        let g = parse("digraph t { a->b }").unwrap();
        assert_eq!(g.edges.len(), 1);
    }

    #[test]
    fn rejects_unsupported_headers() {
        assert!(err("graph t { }").contains("line 1"));
        assert!(err("strict digraph t { }").contains("strict"));
        assert!(err("digraph { }").contains("name"));
    }

    #[test]
    fn errors_name_the_line() {
        assert!(err("digraph t {\n a [x=]\n}").starts_with("line 2"));
        assert!(err("digraph t {\n\n a -> \n}").starts_with("line 4"));
        assert!(err("digraph t {\n a [x=\"open]\n}").starts_with("line 2"));
        assert!(err("digraph t {\n a\n").contains("missing closing"));
        assert!(err("digraph t {\n\n @ }").starts_with("line 3"));
        assert!(err("digraph t { a } b").contains("after the graph"));
    }
}

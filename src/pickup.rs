//! `wteam pickup`: import planning material (`.scratch/<slug>/issues/` from
//! `/to-tickets`, or an existing `.wteam/<slug>/`), then verify it.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::paths::{self, Paths};
use crate::render;
use crate::state::{self, TicketStatus};
use crate::work::{self, Ticket};

/// Fewest tickets at which a serial graph draws a width warning.
const WIDTH_WARN_MIN_TICKETS: usize = 4;

#[derive(Debug, Clone, Serialize)]
pub struct PickupReport {
    pub slug: String,
    pub goal: String,
    pub imported: Vec<String>,
    /// Failed checks (empty means ready to run).
    pub problems: Vec<String>,
    /// Advisory notes that do not block a run.
    pub warnings: Vec<String>,
    /// Tickets on the longest blocker chain.
    pub longest_chain: usize,
    /// Most tickets ready at once.
    pub max_parallel: usize,
    /// Mermaid work graph.
    pub graph: String,
}

/// One warning when the longest chain covers more than half of a graph of 4+ tickets.
fn width_warnings(tickets: usize, w: work::Width) -> Vec<String> {
    if tickets < WIDTH_WARN_MIN_TICKETS || w.longest_chain * 2 <= tickets {
        return Vec::new();
    }
    vec![format!(
        "longest chain covers {} of {tickets} tickets (longest_chain {}, max_parallel {}): \
         drop blocking edges that need no code from another ticket",
        w.longest_chain, w.longest_chain, w.max_parallel
    )]
}

/// One warning per ticket whose body has no `**Context:**` section.
fn missing_context(tickets: &[Ticket]) -> Vec<String> {
    tickets
        .iter()
        .filter(|t| {
            !t.body
                .lines()
                .any(|l| l.trim_start().starts_with("**Context:**"))
        })
        .map(|t| format!("ticket {} has no **Context:** section", t.id))
        .collect()
}

/// Import from `from` (a `.scratch/<slug>` dir) into `paths`, or verify `paths` in place when `from` is None.
pub fn pickup(paths: &Paths, from: Option<&Path>) -> Result<PickupReport> {
    paths::init_effort(paths)?;
    let imported = match from {
        Some(dir) => import(paths, dir)?,
        None => Vec::new(),
    };
    let tickets = work::load_tickets(&paths.tickets_dir())?;
    let st = state::load(paths)?;

    let mut problems = work::validate(&tickets);
    problems.extend(missing_verify_programs(&tickets));
    problems.extend(dirty_tree(&paths.root, from));

    let spec = std::fs::read_to_string(paths.spec_file()).unwrap_or_default();
    let goal = spec_goal(&spec)
        .or_else(|| tickets.first().map(|t| t.title.clone()))
        .unwrap_or_default();

    let width = work::width(&tickets);
    let mut warnings = width_warnings(tickets.len(), width);
    warnings.extend(missing_context(&tickets));
    Ok(PickupReport {
        slug: paths.slug.clone(),
        goal,
        imported,
        problems,
        warnings,
        longest_chain: width.longest_chain,
        max_parallel: width.max_parallel,
        graph: render::work_mermaid(&tickets, &st),
    })
}

/// Sorted `*.md` files directly inside `dir` (empty when `dir` is missing).
fn markdown_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("md"))
        .collect();
    files.sort();
    files
}

/// Copy `issues/*.md` and the spec from `from`; skip tickets whose state is not `todo`.
fn import(paths: &Paths, from: &Path) -> Result<Vec<String>> {
    let issues = from.join("issues");
    if !issues.is_dir() {
        bail!("{} has no issues/ directory", from.display());
    }
    let st = state::load(paths)?;
    let mut imported = Vec::new();
    for src in markdown_files(&issues) {
        let bytes = std::fs::read(&src).with_context(|| format!("reading {}", src.display()))?;
        let md = String::from_utf8_lossy(&bytes);
        let ticket = work::parse_ticket(&md, &src)?;
        let started = st
            .tickets
            .get(&ticket.id)
            .is_some_and(|t| t.status != TicketStatus::Todo);
        if started {
            continue;
        }
        let name = src.file_name().context("ticket file has no name")?;
        let dest = paths.tickets_dir().join(name);
        std::fs::write(&dest, &bytes).with_context(|| format!("writing {}", dest.display()))?;
        imported.push(format!("tickets/{}", name.to_string_lossy()));
    }
    let spec = [from.join("spec.md")]
        .into_iter()
        .filter(|p| p.is_file())
        .chain(markdown_files(from))
        .next();
    if let Some(spec) = spec {
        std::fs::copy(&spec, paths.spec_file())
            .with_context(|| format!("copying {}", spec.display()))?;
        imported.push("spec.md".to_string());
    }
    Ok(imported)
}

/// First paragraph under the spec's `Problem Statement` heading, else under `Solution`.
fn spec_goal(spec: &str) -> Option<String> {
    ["problem statement", "solution"]
        .iter()
        .find_map(|want| section_paragraph(spec, want))
}

fn section_paragraph(spec: &str, heading: &str) -> Option<String> {
    let mut lines = spec.lines().skip_while(|l| {
        let t = l.trim_start();
        !(t.starts_with('#')
            && t.trim_start_matches('#')
                .trim()
                .eq_ignore_ascii_case(heading))
    });
    lines.next()?;
    let mut para: Vec<&str> = Vec::new();
    for line in lines {
        let t = line.trim();
        if t.starts_with('#') {
            break;
        }
        if t.is_empty() {
            if para.is_empty() {
                continue;
            }
            break;
        }
        para.push(t);
    }
    (!para.is_empty()).then(|| para.join("\n"))
}

/// Whether `program` resolves to a file: directly when it has a `/`, else on `PATH`.
fn on_path(program: &str) -> bool {
    if program.contains('/') {
        return Path::new(program).is_file();
    }
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .any(|dir| dir.join(program).is_file())
}

/// Shell builtins and keywords that need no installed program.
const SHELL_BUILTINS: &[&str] = &[
    "cd", "export", "test", "[", "[[", "true", "false", "source", ".", "set", "echo", ":", "exit",
    "unset",
];

/// The program of each `&&`, `||`, `|` or `;` separated segment of a shell command.
fn programs(cmd: &str) -> Vec<&str> {
    cmd.split(['|', '&', ';'])
        .filter_map(|seg| {
            seg.split_whitespace()
                .find(|tok| !tok.contains('=') || tok.starts_with('='))
        })
        .collect()
}

/// One problem per `**Verify:**` program that is neither a builtin nor installed.
fn missing_verify_programs(tickets: &[Ticket]) -> Vec<String> {
    let mut problems = Vec::new();
    for t in tickets {
        for cmd in &t.verify {
            for program in programs(cmd) {
                if !SHELL_BUILTINS.contains(&program) && !on_path(program) {
                    problems.push(format!(
                        "ticket {}: verify program `{program}` not found on PATH",
                        t.id
                    ));
                }
            }
        }
    }
    problems
}

/// Whether `path` is `prefix` or lies under it.
fn under(path: &str, prefix: &str) -> bool {
    let prefix = prefix.trim_end_matches('/');
    path.trim_end_matches('/') == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// Problems for uncommitted changes outside `.wteam/`, `.scratch/` and the `from` source dir.
fn dirty_tree(root: &Path, from: Option<&Path>) -> Vec<String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "--no-optional-locks",
            "status",
            "--porcelain",
            "-z",
            "--untracked-files=all",
        ])
        .output();
    let out = match out {
        Ok(o) if o.status.success() => o,
        _ => return vec![format!("git status failed in {}", root.display())],
    };
    let mut ignored = vec![".wteam".to_string(), ".scratch".to_string()];
    if let (Ok(root), Some(Ok(from))) = (root.canonicalize(), from.map(Path::canonicalize))
        && let Ok(rel) = from.strip_prefix(&root)
    {
        ignored.push(rel.to_string_lossy().to_string());
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut entries = text.split('\0').filter(|e| !e.is_empty());
    let mut problems = Vec::new();
    while let Some(entry) = entries.next() {
        let Some(path) = entry.get(3..) else { continue };
        // Renames and copies carry the old path as a separate entry.
        if entry.starts_with(['R', 'C']) || entry[1..].starts_with(['R', 'C']) {
            entries.next();
        }
        if !ignored.iter().any(|i| under(path, i)) {
            problems.push(format!("git tree is dirty: {path}"));
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goal_prefers_problem_statement_over_solution() {
        let spec =
            "# S\n\n## Solution\n\nfix it\n\n## Problem Statement\n\nit breaks\nbadly\n\nmore\n";
        assert_eq!(spec_goal(spec).as_deref(), Some("it breaks\nbadly"));
    }

    #[test]
    fn goal_uses_solution_and_none_without_either() {
        assert_eq!(
            spec_goal("## Solution\n\n\nfix it\n## Next\nx").as_deref(),
            Some("fix it")
        );
        assert_eq!(spec_goal("# S\n\nnothing\n"), None);
    }

    #[test]
    fn programs_split_on_shell_operators() {
        assert_eq!(
            programs("cd x && cargo test | tee out || FOO=1 npm run a"),
            vec!["cd", "cargo", "tee", "npm"]
        );
    }

    #[test]
    fn under_matches_whole_path_components() {
        assert!(under(".scratch/", ".scratch"));
        assert!(under(".scratch/a/b", ".scratch"));
        assert!(!under(".scratchy", ".scratch"));
    }

    #[test]
    fn on_path_finds_sh_and_rejects_nonsense() {
        assert!(on_path("sh"));
        assert!(!on_path("no-such-program-xyz"));
    }
}

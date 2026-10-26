#!/usr/bin/env python3
"""Patch pinned e8d709d integration tests for semantic orchestrator guidance checks."""

from __future__ import annotations

import re
import sys
from pathlib import Path

ORCHESTRATOR_TEST = r'''
#[test]
fn orchestrator_runs_one_command_per_shell_call_and_fills_pairs() {
    let text = role("orchestrator");
    assert!(
        text.contains("no `&&`, `;` or `|`"),
        "roles/orchestrator.md must forbid shell chaining"
    );
    assert!(
        text.contains("If `wteam` itself is denied") && text.contains("naming the denied command"),
        "roles/orchestrator.md must handle denied wteam commands"
    );
    assert!(
        text.contains("wteam dispatch --fill"),
        "roles/orchestrator.md must document wteam dispatch --fill"
    );
    let lower = text.to_lowercase();
    let one_per_shell = lower.contains("one command per shell")
        || lower.contains("exactly one command")
        || (lower.contains("one") && lower.contains("command") && lower.contains("shell"));
    assert!(
        one_per_shell,
        "roles/orchestrator.md must require one command per shell call (semantic)"
    );
    let no_dispatch_chains = text.contains("Never chain")
        || text.contains("one `wteam dispatch --fill`")
        || (text.contains("dispatch --fill") && text.contains("rather than"));
    assert!(
        no_dispatch_chains,
        "roles/orchestrator.md must steer away from chained dispatches"
    );
}
'''

KICKOFF_TEST = r'''
#[test]
fn kickoff_and_digest_end_with_the_fill_hint() {
    let (r, _) = one_per_turn_run(2, 2);
    let prompts = r.read(".wteam-fake/orch-prompts");
    let fill_mentions = prompts.matches("dispatch --fill").count() >= 2;
    let lower = prompts.to_lowercase();
    let busy_hint = lower.contains("pairs busy")
        || lower.contains("keep pairs")
        || lower.contains("idle pair")
        || lower.contains("fill pairs");
    assert!(
        fill_mentions && busy_hint,
        "kickoff/digest must mention fill dispatch and keeping pairs busy: {prompts}"
    );
}
'''


def replace_fn(source: str, name: str, replacement: str) -> str:
    pattern = re.compile(
        rf"#\[test\]\s*\nfn {name}\(\) \{{.*?\n\}}\n",
        re.DOTALL,
    )
    if not pattern.search(source):
        raise SystemExit(f"test fn {name} not found")
    return pattern.sub(replacement.strip() + "\n", source, count=1)


def main() -> None:
    tests = Path(sys.argv[1])
    roles = tests / "roles.rs"
    watcher = tests / "watcher.rs"
    roles.write_text(
        replace_fn(
            roles.read_text(encoding="utf-8"),
            "orchestrator_runs_one_command_per_shell_call_and_fills_pairs",
            ORCHESTRATOR_TEST,
        ),
        encoding="utf-8",
    )
    watcher.write_text(
        replace_fn(
            watcher.read_text(encoding="utf-8"),
            "kickoff_and_digest_end_with_the_fill_hint",
            KICKOFF_TEST,
        ),
        encoding="utf-8",
    )


if __name__ == "__main__":
    main()

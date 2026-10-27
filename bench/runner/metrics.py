"""Aggregate bench agent metrics from Claude JSON, Cursor stream-json, and wteam logs."""

from __future__ import annotations

import glob
import json
import os
import sys
from pathlib import Path

CURSOR_FIELDS = ("inputTokens", "cacheReadTokens", "cacheWriteTokens", "outputTokens")


def claude_result(path: Path) -> dict:
    try:
        raw = json.loads(path.read_text(encoding="utf-8", errors="replace"))
    except (OSError, json.JSONDecodeError):
        return {}
    if isinstance(raw, list):
        raw = next((e for e in reversed(raw) if e.get("type") == "result"), {})
    if not isinstance(raw, dict):
        return {}
    u = raw.get("usage") or {}
    inp = (
        u.get("input_tokens", 0)
        + u.get("cache_read_input_tokens", 0)
        + u.get("cache_creation_input_tokens", 0)
    )
    return {
        "cost_usd": raw.get("total_cost_usd") or 0,
        "turns": 1,
        "input_tokens": inp,
        "output_tokens": u.get("output_tokens") or 0,
        "denials": len(raw.get("permission_denials") or []),
    }


def scan_cursor_file(path: Path) -> dict:
    totals = {f: 0 for f in CURSOR_FIELDS}
    turns = 0
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        try:
            ev = json.loads(line)
        except json.JSONDecodeError:
            continue
        if ev.get("type") != "result":
            continue
        turns += 1
        usage = ev.get("usage") or {}
        if isinstance(usage, dict):
            for f in CURSOR_FIELDS:
                totals[f] += usage.get(f, 0)
    if turns == 0:
        return {}
    return {
        "turns": turns,
        "total_tokens": sum(totals.values()),
        **{f: totals[f] for f in CURSOR_FIELDS},
    }


def scan_wteam_claude_logs(repo: Path) -> dict:
    cost = 0.0
    turns = 0
    denials = 0
    for f in glob.glob(str(repo / ".wteam/bench/logs/*-[0-9]*.log")):
        for line in Path(f).read_text(encoding="utf-8", errors="replace").splitlines():
            try:
                ev = json.loads(line)
            except json.JSONDecodeError:
                continue
            if ev.get("type") != "result":
                continue
            turns += 1
            cost += ev.get("total_cost_usd") or 0
            denials += len(ev.get("permission_denials") or [])
    if turns == 0:
        return {}
    return {"cost_usd": round(cost, 4), "agent_turns": turns, "permission_denials": denials}


def main() -> None:
    logs = Path(sys.argv[1])
    repo = Path(sys.argv[2]) if len(sys.argv) > 2 else None

    cost = 0.0
    turns = 0
    denials = 0
    input_tok = 0
    output_tok = 0
    cursor_total = 0
    cursor_turns = 0

    if logs.is_dir():
        for path in sorted(logs.glob("*")):
            if path.suffix == ".json":
                r = claude_result(path)
                cost += r.get("cost_usd") or 0
                turns += r.get("turns") or 0
                input_tok += r.get("input_tokens") or 0
                output_tok += r.get("output_tokens") or 0
                denials += r.get("denials") or 0
            elif path.suffix == ".jsonl":
                r = scan_cursor_file(path)
                if r:
                    cursor_turns += r.get("turns") or 0
                    cursor_total += r.get("total_tokens") or 0

    if repo and repo.is_dir():
        w = scan_wteam_claude_logs(repo)
        if w:
            cost += w.get("cost_usd") or 0
            turns += w.get("agent_turns") or 0
            denials += w.get("permission_denials") or 0
        for path in sorted((repo / ".wteam/bench/logs").glob("*")) if (repo / ".wteam/bench/logs").is_dir() else []:
            if path.suffix in (".jsonl", ".log"):
                r = scan_cursor_file(path)
                if r:
                    cursor_turns += r.get("turns") or 0
                    cursor_total += r.get("total_tokens") or 0

    if cost:
        print(f"cost_usd {round(cost, 4)}")
    if turns and not cursor_turns:
        print(f"agent_turns {turns}")
    if input_tok or output_tok:
        print(f"input_tokens {input_tok}")
        print(f"output_tokens {output_tok}")
    if denials:
        print(f"permission_denials {denials}")
    if cursor_turns:
        print(f"agent_turns {cursor_turns}")
        print(f"total_tokens {cursor_total}")


if __name__ == "__main__":
    main()

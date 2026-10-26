"""Count Cursor result-event tokens across every brownfield agent turn."""

import json
import sys
from pathlib import Path


FIELDS = ("inputTokens", "cacheReadTokens", "cacheWriteTokens", "outputTokens")


def files(paths: list[str]):
    for raw in paths:
        path = Path(raw)
        if path.is_dir():
            yield from sorted(path.glob("*.jsonl"))
            yield from sorted(path.glob("*-[0-9]*.log"))
        else:
            yield path


def main() -> None:
    totals = {field: 0 for field in FIELDS}
    turns = 0
    missing_usage = 0
    for path in files(sys.argv[1:]):
        with path.open(encoding="utf-8", errors="replace") as stream:
            for line in stream:
                try:
                    event = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if not isinstance(event, dict) or event.get("type") != "result":
                    continue
                turns += 1
                usage = event.get("usage")
                if not isinstance(usage, dict):
                    missing_usage += 1
                    continue
                for field in FIELDS:
                    totals[field] += usage.get(field, 0)
    if turns == 0:
        raise SystemExit("no Cursor result events found")
    print(f"agent_turns {turns}")
    for field in FIELDS:
        print(f"{field} {totals[field]}")
    print(f"total_tokens {sum(totals.values())}")
    print(f"turns_missing_usage {missing_usage}")


if __name__ == "__main__":
    main()

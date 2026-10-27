#!/bin/sh
# Placeholder: Harbor has no built-in wteam agent yet.
# Team comparisons on TB 4.0 need a custom Harbor agent (BaseInstalledAgent) that
# wraps wteam in the task container or host. Until then, use bench/brownfield run_team.
set -eu
echo "Terminal-Bench team arm: not wired — implement harbor agent import path for wteam." >&2
echo "See bench/terminal-bench/README.md" >&2
exit 2

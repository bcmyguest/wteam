#!/bin/sh
# Delegate to harness-tb (wteam team arm on Terminal-Bench tasks).
exec sh "$(cd "$(dirname "$0")" && pwd)/harness/run_team.sh" "$@"

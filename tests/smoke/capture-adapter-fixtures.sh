#!/usr/bin/env bash
# Capture live OpenCode/Pi JSONL for tests/fixtures/adapters/. Run from repo root:
#   tests/smoke/capture-adapter-fixtures.sh
#
# OpenCode: uses a configured free/cheap model (override with WTEAM_OPENCODE_SMOKE_MODEL).
# Pi: experimental — skipped unless WTEAM_PI_SMOKE=1 and `pi` is on PATH with credentials.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

OPENCODE_FIXTURE="$ROOT/tests/fixtures/adapters/opencode/first-turn.jsonl"
PI_FIXTURE="$ROOT/tests/fixtures/adapters/pi/first-turn.jsonl"
PROMPT='Reply with exactly one word: WTEAM_OK'
MODEL="${WTEAM_OPENCODE_SMOKE_MODEL:-github-copilot/gemini-3.8-flash}"
TIMEOUT="${WTEAM_SMOKE_TIMEOUT:-180}"

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

capture_opencode() {
  if ! command -v opencode >/dev/null 2>&1; then
    echo "skip opencode: not on PATH" >&2
    return 0
  fi
  local out="$tmpdir/opencode.jsonl"
  echo "opencode smoke: model=$MODEL" >&2
  if ! timeout "$TIMEOUT" opencode run --format json --auto -m "$MODEL" --dir "$tmpdir" -- "$PROMPT" \
    >"$out" 2>"$tmpdir/opencode.err"; then
    echo "opencode smoke failed (see $tmpdir/opencode.err):" >&2
    tail -20 "$tmpdir/opencode.err" >&2 || true
    return 1
  fi
  if ! grep -q '"type":"step_finish"' "$out" && ! grep -q '"type": "step_finish"' "$out"; then
    echo "opencode smoke: no step_finish in output; not updating fixture" >&2
    head -5 "$out" >&2 || true
    return 1
  fi
  cp "$out" "$OPENCODE_FIXTURE"
  echo "updated $OPENCODE_FIXTURE ($(wc -l <"$out") lines)" >&2
}

capture_pi() {
  if [[ "${WTEAM_PI_SMOKE:-}" != "1" ]]; then
    echo "skip pi: experimental (set WTEAM_PI_SMOKE=1 to capture)" >&2
    return 0
  fi
  if ! command -v pi >/dev/null 2>&1; then
    echo "skip pi: not on PATH" >&2
    return 0
  fi
  local out="$tmpdir/pi.jsonl"
  echo "pi smoke: experimental capture" >&2
  if ! (cd "$tmpdir" && timeout "$TIMEOUT" pi --mode json -- "$PROMPT" >"$out" 2>"$tmpdir/pi.err"); then
    echo "pi smoke failed (see $tmpdir/pi.err):" >&2
    tail -20 "$tmpdir/pi.err" >&2 || true
    return 1
  fi
  if ! grep -q '"type":"message_end"' "$out" && ! grep -q '"type": "message_end"' "$out"; then
    echo "pi smoke: no assistant message_end; not updating fixture" >&2
    head -5 "$out" >&2 || true
    return 1
  fi
  cp "$out" "$PI_FIXTURE"
  echo "updated $PI_FIXTURE ($(wc -l <"$out") lines)" >&2
}

status=0
capture_opencode || status=1
capture_pi || status=1
exit "$status"

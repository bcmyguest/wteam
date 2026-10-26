# Shared brownfield harness helpers. Source from run_*.sh after HERE= is set.
# shellcheck shell=sh

require_cmd() {
  for c; do
    command -v "$c" >/dev/null 2>&1 || {
      echo "$c not on PATH" >&2
      exit 1
    }
  done
}

ensure_bench_wteam() {
  ROOT=$(cd "$(dirname "$0")/../.." && pwd)
  cargo build --release -q --manifest-path "$ROOT/Cargo.toml"
  export PATH="$ROOT/target/release:$PATH"
  export WTEAM="$ROOT/target/release/wteam"
}

wteam_bin() {
  if [ -n "${WTEAM:-}" ]; then
    printf '%s\n' "$WTEAM"
    return
  fi
  if command -v wteam >/dev/null 2>&1; then
    printf 'wteam\n'
    return
  fi
  ROOT=$(cd "$(dirname "$0")/../.." && pwd)
  if [ -x "$ROOT/target/release/wteam" ]; then
    printf '%s\n' "$ROOT/target/release/wteam"
    return
  fi
  echo "wteam not on PATH (set WTEAM= or cargo build --release)" >&2
  exit 1
}

install_bench_gate() {
  OUT=$1
  BENCH=$2
  mkdir -p "$OUT/.wteam/bench" "$OUT/.wteam/roles"
  cat >"$OUT/.wteam/bench/hidden_gate.sh" <<EOF
#!/bin/sh
# Brownfield benchmark hidden gate (pinned tests + fmt). Invoked by validate-out.
set -eu
ROOT=\$(cd "\$(dirname "\$0")/../.." && pwd)
exec sh "$BENCH/grade.sh" "\$ROOT"
EOF
  chmod +x "$OUT/.wteam/bench/hidden_gate.sh"
  cat >"$OUT/.wteam/roles/validate-out.md" <<'EOF'
# validate-out (brownfield benchmark)

Fresh session — outcome validation. You never saw the planning or implementation
chats.

## Job

1. Read the Outcome in `spec.md` if present. The **authoritative full gate** for
   this benchmark is exactly one command (not plain `cargo test` on the checkout):
   `sh .wteam/bench/hidden_gate.sh`
   It runs `cargo fmt --check` and the pinned integration suite the grader uses.
2. Map each failure to ticket id(s) the orchestrator should `requeue`.
3. FAIL with concrete `- ` bullets (ticket id + what broke). PASS only when the
   hidden gate exits 0 and the Outcome looks met.

## Rules

- Do not edit source code. Write only under `.wteam/` (findings file).
- Run exactly one command per shell call: no `&&`, `;` or `|`.
- Finish with exactly one of:
  - `wteam phase-verdict pass --file <path>`
  - `wteam phase-verdict fail --file <path>`
- The file must contain a `FINDINGS:` section. FAIL needs at least one bullet.
EOF
}

finish_metrics() {
  LOGS=$1
  OUT=$2
  BENCH=$3
  grep -E "^Run stopped" "$LOGS/run.txt" 2>/dev/null >>"$LOGS/metrics.txt" || true
  if [ -f "$BENCH/audit.sh" ]; then
    OWN="$OUT" sh "$BENCH/audit.sh" "$LOGS" "$OUT/.wteam" >"$LOGS/audit.txt" 2>/dev/null || true
    head -1 "$LOGS/audit.txt" 2>>"$LOGS/metrics.txt" || true
  fi
}

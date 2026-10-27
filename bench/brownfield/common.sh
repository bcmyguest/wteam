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

install_bench_brief() {
  OUT=$1
  mkdir -p "$OUT/.wteam"
  cat >"$OUT/.wteam/brief.md" <<'EOF'
# Brownfield bench

- Use `wteam` from PATH (harness release). Do not `cargo run` or build wteam from this checkout.
- Land ticket work on **main** with normal commits; the hidden grader reads the main tree.
- When `.wteam/bench/hidden_gate.sh` exists, run it for the full verification gate.
EOF
}

install_bench_docs() {
  OUT=$1
  [ -f "$OUT/CLAUDE.md" ] || return 0
  if ! grep -q '## Brownfield bench' "$OUT/CLAUDE.md" 2>/dev/null; then
    cat >>"$OUT/CLAUDE.md" <<'EOF'

## Brownfield bench

Use the harness `wteam` on PATH for every agent command. This checkout is a historical snapshot; do not run wteam from `cargo` here.
EOF
    (cd "$OUT" && git add CLAUDE.md && git commit -qm "docs: bench uses harness wteam on PATH")
  fi
}

install_autopass_reviewer() {
  OUT=$1
  mkdir -p "$OUT/.wteam/roles" "$OUT/.wteam-fake"
  cat >"$OUT/.wteam/roles/autopass.md" <<'EOF'
# autopass reviewer (brownfield bench)

Run exactly one command: `wteam verdict NN pass` with NN the ticket id from your message.
EOF
  cat >"$OUT/.wteam-fake/reviewer.sh" <<'EOF'
#!/bin/sh
set -eu
id=$(printf '%s\n' "$WTEAM_PROMPT" | sed -n 's/.*[Tt]icket \([0-9][0-9]\).*/\1/p' | head -1)
[ -n "$id" ] || { echo "no ticket id in prompt"; exit 1; }
wteam verdict "$id" pass
EOF
  chmod +x "$OUT/.wteam-fake/reviewer.sh"
  export WTEAM_FAKE_DIR="$OUT/.wteam-fake"
}

write_reviewer_agent() {
  OUT=$1
  MODEL=$2
  REVIEW_MODE=${WTEAM_BENCH_REVIEW:-autopass}
  if [ "$REVIEW_MODE" = cursor ]; then
    printf 'name = "reviewer"\ncli = "cursor"\nmodel = "%s"\nprompt = "reviewer"\nprofile = "review"\nextra_args = ["--force"]\n' \
      "$MODEL" >"$OUT/.wteam/agents/reviewer.toml"
  else
    install_autopass_reviewer "$OUT"
    printf 'name = "reviewer"\ncli = "fake"\nmodel = "fake"\nprompt = "autopass"\nprofile = "review"\n' \
      >"$OUT/.wteam/agents/reviewer.toml"
  fi
}

install_bench_plan_review() {
  OUT=$1
  mkdir -p "$OUT/.wteam/roles"
  cat >"$OUT/.wteam/roles/spec-critic.md" <<'EOF'
# spec-critic (brownfield benchmark)

Fresh session — antagonistic review of a wteam plan. You never saw the planner's chat.

## Job

1. Read `spec.md` and every ticket under `tickets/`. You may Read the repo to check paths.
2. Hunt for **blocking** plan defects only: bad `Blocked by`, missing Files/Verify, contradictions, wrong Context.
3. FAIL with concrete `- ` bullets when an implementer would be blocked or misled. PASS when coherent enough to build.

## Brownfield Verify rules

This repo has `sh .wteam/bench/hidden_gate.sh` (pinned fmt + integration suite).

- **PASS** Verify that runs a whole test binary (`cargo test --test watcher`, `cargo test --test parallel fill_`, `cargo test --test docs`) or the hidden gate. Do not FAIL because Verify omits every substring test name.
- **PASS** a docs ticket whose Verify is `cargo test --test docs` when another ticket or the spec names `hidden_gate.sh` for the full outcome.
- **FAIL** Verify that is empty, not runnable, or points at commands that do not exist in the repo.

Put speculative coupling under `NOTES:`, not FINDINGS.

## Rules

- Do not edit source code. Write only under `.wteam/` (findings file).
- Run exactly one command per shell call: no `&&`, `;` or `|`.
- Finish with `wteam phase-verdict pass|fail --file <path>` (or plan-verdict for plan-review).
- The file must contain `FINDINGS:`; FAIL needs at least one bullet.
EOF
}

install_bench_gate() {
  OUT=$1
  BENCH=$2
  install_bench_brief "$OUT"
  install_bench_plan_review "$OUT"
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

brownfield_goal() {
  BENCH=$1
  cat "$BENCH/task.md"
}

brownfield_plan_prompt() {
  wteam=$1
  root=$2
  goal=$3
  pairs=$4
  printf '%s\n' "$("$wteam" --root "$root" --slug bench role planner)

Goal:
$goal

Slug: bench
Write the spec and 2-6 implementation tickets under .wteam/bench/. Use the existing code and tests for Context and Verify. Finish only after wteam --slug bench pickup reports no problems."
}

brownfield_finish_run() {
  LOGS=$1
  OUT=$2
  BENCH=$3
  WTEAM=$(wteam_bin)
  python3 "$BENCH/token_usage.py" "$LOGS" "$OUT/.wteam/bench/logs" >>"$LOGS/metrics.txt" 2>/dev/null || \
    python3 "$BENCH/token_usage.py" "$LOGS" >>"$LOGS/metrics.txt" 2>/dev/null || true
  finish_metrics "$LOGS" "$OUT" "$BENCH"
  if sh "$BENCH/grade.sh" "$OUT" >"$LOGS/grade.txt" 2>&1; then
    echo "grade_pass 1" >>"$LOGS/metrics.txt"
    tail -n 5 "$LOGS/grade.txt"
  else
    echo "grade_pass 0" >>"$LOGS/metrics.txt"
    "$WTEAM" --root "$OUT" --slug bench refine --logs "$LOGS" --grade "$LOGS/grade.txt" --run-log "$LOGS/run.txt" \
      >"$LOGS/refine.txt" 2>&1 || true
    tail -n 12 "$LOGS/grade.txt"
    return 1
  fi
}

bench_export_models() {
  export BENCH_CLI=${BENCH_CLI:-cursor}
  export BENCH_MODEL=${BENCH_MODEL:-${WTEAM_BENCH_MODEL:-composer-2.5}}
  export BENCH_WORKER_MODEL=${BENCH_WORKER_MODEL:-${WTEAM_BENCH_WORKER_MODEL:-$BENCH_MODEL}}
  export BENCH_REVIEWER_MODEL=${BENCH_REVIEWER_MODEL:-${WTEAM_BENCH_REVIEWER_MODEL:-$BENCH_MODEL}}
  export BENCH_REVIEW=${BENCH_REVIEW:-${WTEAM_BENCH_REVIEW:-autopass}}
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

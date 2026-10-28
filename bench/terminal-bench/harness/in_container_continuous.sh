#!/bin/sh
# One agent session on the full TB instruction (harness continuous arm).
set -eu
INSTRUCTION=$1
MODEL=${BENCH_MODEL:-auto}
CLI=${BENCH_CLI:-cursor}
cd /app
export PATH="/usr/local/bin:/opt/cursor-agent:${PATH:-/usr/bin:/bin}"
export HOME="${HOME:-/root}"

# Prefer tools + artifacts over long free-form generation. Reward requires the
# hidden verifier to pass — writing a file is not enough.
PREAMBLE='Terminal-Bench harness notes:
- Work only under /app. Prefer short tool calls (read/write/edit/bash) over long narration.
- Produce every required output file, then VALIDATE before stopping.
- If /app/check_routing.py exists: run `python3 /app/check_routing.py` and fix until exit 0.
- If `/app/tests` exists: run `npm test` until green. Also run `npm run build` and `npm run submit` when those scripts exist.
- Export every symbol the tests import (e.g. named `submitLead` from `/app/src/lib/submitLead.ts`).
- Keep reasoning brief; do not fill the context with analysis that is not written to disk.
'

last_step_finish_reason() {
  awk '
    /"type"[[:space:]]*:[[:space:]]*"step_finish"/ {
      if (match($0, /"reason"[[:space:]]*:[[:space:]]*"[^"]+"/)) {
        s = substr($0, RSTART, RLENGTH)
        sub(/^.*"reason"[[:space:]]*:[[:space:]]*"/, "", s)
        sub(/".*/, "", s)
        last = s
      }
    }
    END { print last }
  ' "$1" 2>/dev/null || true
}

consecutive_length_count() {
  awk '
    /"type"[[:space:]]*:[[:space:]]*"step_finish"/ {
      if (match($0, /"reason"[[:space:]]*:[[:space:]]*"[^"]+"/)) {
        s = substr($0, RSTART, RLENGTH)
        sub(/^.*"reason"[[:space:]]*:[[:space:]]*"/, "", s)
        sub(/".*/, "", s)
        if (s == "length") n++
        else n = 0
      }
    }
    END { print n+0 }
  ' "$1" 2>/dev/null || echo 0
}

log_has_write_tool() {
  awk '
    /"type"[[:space:]]*:[[:space:]]*"tool_use"/ || /"type"[[:space:]]*:[[:space:]]*"tool"/ {
      if ($0 ~ /"tool"[[:space:]]*:[[:space:]]*"(write|edit|bash|shell|file_write)"/) found=1
    }
    END { exit(found ? 0 : 1) }
  ' "$1" 2>/dev/null
}

# Inject Harbor verifier suites when the agent image ships none (e.g. react-lead-form).
# Without this, `vitest run` exits 0 with "No test files" — a false green that skips fix loops.
inject_mounted_tests() {
  [ -d /opt/wteam-harness/tb-verify-tests ] || return 1
  mkdir -p /app/tests
  # Copy unit suites the agent/npm test can see.
  for f in lead-form.test.tsx lead-form-verifier.test.tsx vitest.verifier.config.mts test_outputs.mjs; do
    [ -f "/opt/wteam-harness/tb-verify-tests/$f" ] || continue
    cp "/opt/wteam-harness/tb-verify-tests/$f" "/app/tests/$f"
  done
  # Also stage under /tests for test_outputs path compatibility.
  mkdir -p /tests
  cp -a /opt/wteam-harness/tb-verify-tests/. /tests/ 2>/dev/null || true
  return 0
}

# react-lead-form: mirror Harbor test.sh enough that exit 0 ≈ reward 1.
run_react_lead_verify() {
  inject_mounted_tests || true
  cd /app
  npm test > /tmp/npm_test.log 2>&1
  echo $? > /tmp/exit_npm_test.txt
  if [ -f /app/tests/vitest.verifier.config.mts ] || [ -f /tests/vitest.verifier.config.mts ]; then
    cfg=/app/tests/vitest.verifier.config.mts
    [ -f "$cfg" ] || cfg=/tests/vitest.verifier.config.mts
    cp "$cfg" /app/vitest.verifier.config.mts
    npx vitest run --config /app/vitest.verifier.config.mts > /tmp/verifier_vitest.log 2>&1
    echo $? > /tmp/exit_verifier_vitest.txt
  else
    echo 1 > /tmp/exit_verifier_vitest.txt
  fi
  npm run build > /tmp/npm_build.log 2>&1
  echo $? > /tmp/exit_npm_build.txt
  rm -rf /app/output
  npm run submit > /tmp/npm_submit1.log 2>&1
  echo $? > /tmp/exit_npm_submit1.txt
  cp /app/output/submission.json /tmp/submission_run1.json 2>/dev/null || true
  rm -rf /app/output
  npm run submit > /tmp/npm_submit2.log 2>&1
  echo $? > /tmp/exit_npm_submit2.txt
  echo "===== npm test ====="; cat /tmp/npm_test.log
  echo "===== verifier vitest ====="; cat /tmp/verifier_vitest.log 2>/dev/null || true
  echo "===== build ====="; cat /tmp/npm_build.log
  echo "===== submit1 ====="; cat /tmp/npm_submit1.log
  echo "===== submit2 ====="; cat /tmp/npm_submit2.log
  outs=/tests/test_outputs.mjs
  [ -f "$outs" ] || outs=/app/tests/test_outputs.mjs
  if [ -f "$outs" ]; then
    node "$outs"
  else
    # Fall back: require unit tests + build + submit green.
    [ "$(cat /tmp/exit_npm_test.txt)" = 0 ] \
      && [ "$(cat /tmp/exit_npm_build.txt)" = 0 ] \
      && [ "$(cat /tmp/exit_npm_submit1.txt)" = 0 ]
  fi
}

# Pick a local self-check command when the task ships one. Exit 0 => likely reward.
detect_verify_cmd() {
  if [ -f /app/check_routing.py ]; then
    printf '%s\n' "python3 /app/check_routing.py"
    return 0
  fi
  # Prefer full Harbor-aligned verify when verifier suites are mounted.
  if [ -f /opt/wteam-harness/tb-verify-tests/test_outputs.mjs ] \
    || [ -f /opt/wteam-harness/tb-verify-tests/lead-form.test.tsx ]; then
    printf '%s\n' "run_react_lead_verify"
    return 0
  fi
  if [ -f /app/package.json ] && grep -q '"test"' /app/package.json 2>/dev/null; then
    # Vacuous vitest (no tests in image) must not count as pass.
    if ! find /app \( -name '*.test.*' -o -name '*.spec.*' \) 2>/dev/null | grep -q .; then
      if inject_mounted_tests \
        && find /app/tests -name '*.test.*' 2>/dev/null | grep -q .; then
        printf '%s\n' "npm test"
        return 0
      fi
      return 1
    fi
    printf '%s\n' "npm test"
    return 0
  fi
  if [ -f /task/lakefile.lean ] || [ -f /task/lake-manifest.json ]; then
    printf '%s\n' "sh -c 'cd /task && lake build GSLean'"
    return 0
  fi
  return 1
}

run_verify() {
  # Prints checker output to stdout; returns checker exit status.
  cmd=$1
  if [ "$cmd" = run_react_lead_verify ]; then
    run_react_lead_verify
    return $?
  fi
  # shellcheck disable=SC2086
  eval "$cmd" 2>&1
}

# Make verifier suites visible before the agent starts (TDD against real checks).
inject_mounted_tests 2>/dev/null || true

case $CLI in
  codex)
    exec codex exec --json --dangerously-bypass-approvals-and-sandbox \
      --skip-git-repo-check --model "$MODEL" --cd /app -- "${PREAMBLE}
${INSTRUCTION}"
    ;;
  cursor)
    exec agent -p --trust --force --output-format stream-json \
      --model "$MODEL" --workspace /app "${PREAMBLE}
${INSTRUCTION}"
    ;;
  opencode)
    case $MODEL in
      */*) OC_MODEL=$MODEL ;;
      *) OC_MODEL="lemonade/$MODEL" ;;
    esac
    VARIANT=${TB_OPENCODE_VARIANT:-}
    MAX_CONT=${TB_OPENCODE_CONTINUES:-3}
    MAX_FRESH=${TB_OPENCODE_FRESH_RETRIES:-1}
    MAX_LEN_STREAK=${TB_OPENCODE_LENGTH_STREAK:-2}
    MAX_VERIFY=${TB_OPENCODE_VERIFY_RETRIES:-8}
    CONT_MSG='You hit the generation length limit before finishing. Continue with short tool calls only: write/fix required files under /app, run the local checker, then stop only when it passes.'
    FRESH_MSG="${PREAMBLE}
Prior attempt hit the output length limit or stalled. Start fresh: short tool calls, write required outputs, run the local checker, fix until it passes.

TASK:
${INSTRUCTION}"
    set -- opencode run --format json --dangerously-skip-permissions -m "$OC_MODEL"
    if [ -n "$VARIANT" ]; then
      set -- "$@" --variant "$VARIANT"
    fi
    OC_LOG=${TB_OPENCODE_RUN_LOG:-/tmp/opencode-harness.jsonl}
    : >"$OC_LOG"

    kill_opencode_tree() {
      for p in /proc/[0-9]*; do
        exe=$(readlink "$p/exe" 2>/dev/null) || continue
        case $exe in
          */opencode) kill "$(basename "$p")" 2>/dev/null || true ;;
        esac
      done
    }

    watch_length_streak() {
      log=$1
      streak_max=$2
      while kill -0 "$3" 2>/dev/null; do
        n=$(consecutive_length_count "$log")
        if [ "$n" -ge "$streak_max" ]; then
          echo "{\"type\":\"harness_watchdog\",\"reason\":\"length_streak\",\"count\":$n}"
          kill_opencode_tree
          kill "$3" 2>/dev/null || true
          return 0
        fi
        sleep 5
      done
    }

    run_turn() {
      turn_ec=0
      if command -v bash >/dev/null 2>&1; then
        bash -c 'set -o pipefail; "$@" | tee -a "$0"' "$OC_LOG" "$@" &
        turn_pid=$!
      elif (set -o pipefail) 2>/dev/null; then
        set -o pipefail
        "$@" | tee -a "$OC_LOG" &
        turn_pid=$!
      else
        part=${TB_OPENCODE_RUN_PART:-/tmp/opencode-harness.part}
        "$@" >"$part" &
        turn_pid=$!
        wait "$turn_pid" || turn_ec=$?
        tee -a "$OC_LOG" <"$part"
        return "$turn_ec"
      fi
      watch_length_streak "$OC_LOG" "$MAX_LEN_STREAK" "$turn_pid" &
      watch_pid=$!
      wait "$turn_pid" || turn_ec=$?
      kill "$watch_pid" 2>/dev/null || true
      wait "$watch_pid" 2>/dev/null || true
      return "$turn_ec"
    }

    ec=0
    run_turn "$@" -- "${PREAMBLE}
${INSTRUCTION}" || ec=$?

    # 1) Same-session continues while finish reason is length.
    i=0
    while [ "$i" -lt "$MAX_CONT" ]; do
      reason=$(last_step_finish_reason "$OC_LOG")
      [ "$reason" = length ] || break
      i=$((i + 1))
      echo "{\"type\":\"harness_continue\",\"attempt\":$i,\"reason\":\"length\",\"mode\":\"session\"}"
      turn_ec=0
      run_turn "$@" --continue -- "$CONT_MSG" || turn_ec=$?
      ec=$turn_ec
    done

    # 2) Fresh session if still length-truncated or never wrote.
    f=0
    while [ "$f" -lt "$MAX_FRESH" ]; do
      reason=$(last_step_finish_reason "$OC_LOG")
      if [ "$reason" != length ] && log_has_write_tool "$OC_LOG"; then
        break
      fi
      f=$((f + 1))
      echo "{\"type\":\"harness_continue\",\"attempt\":$f,\"reason\":\"${reason:-none}\",\"mode\":\"fresh\"}"
      turn_ec=0
      run_turn "$@" -- "$FRESH_MSG" || turn_ec=$?
      ec=$turn_ec
    done

    # JSON-escape a string without requiring python3 (Node-only TB images).
    json_str() {
      if command -v python3 >/dev/null 2>&1; then
        printf '%s' "$1" | python3 -c 'import json,sys; print(json.dumps(sys.stdin.read()), end="")'
        return
      fi
      if command -v node >/dev/null 2>&1; then
        printf '%s' "$1" | node -e 'let s="";process.stdin.on("data",d=>s+=d);process.stdin.on("end",()=>process.stdout.write(JSON.stringify(s)))'
        return
      fi
      # Minimal fallback.
      printf '"'
      printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g; s/	/\\t/g' | tr '\n' ' '
      printf '"'
    }

    snip_verify_out() {
      # Prefer FAIL/ERROR lines; else head. No python required.
      f=$1
      if grep -E 'FAIL|ERROR|Error|Assertion|Expected|✕|×' "$f" >/tmp/harness-verify-snip 2>/dev/null; then
        :
      else
        head -n 60 "$f" >/tmp/harness-verify-snip 2>/dev/null || : > /tmp/harness-verify-snip
      fi
      head -c 3500 /tmp/harness-verify-snip 2>/dev/null || true
    }

    # 3) Verify→fix: reward requires the checker/tests to pass, not just a file.
    VERIFY_CMD=
    if VERIFY_CMD=$(detect_verify_cmd); then
      v=0
      while [ "$v" -lt "$MAX_VERIFY" ]; do
        verify_out=/tmp/harness-verify.out
        verify_ec=0
        run_verify "$VERIFY_CMD" >"$verify_out" || verify_ec=$?
        echo "{\"type\":\"harness_verify\",\"attempt\":$((v + 1)),\"cmd\":$(json_str "$VERIFY_CMD"),\"exit\":$verify_ec}"
        if [ "$verify_ec" -eq 0 ]; then
          echo "{\"type\":\"harness_verify\",\"status\":\"pass\"}"
          ec=0
          break
        fi
        v=$((v + 1))
        err_snip=$(snip_verify_out "$verify_out")
        echo "{\"type\":\"harness_continue\",\"attempt\":$v,\"reason\":\"verify_fail\",\"mode\":\"verify\"}"
        turn_ec=0
        run_turn "$@" --continue -- "Local checker failed (exit $verify_ec). Use short tool calls only (read/edit/write/bash). No long narration. Fix the code, then re-run: $VERIFY_CMD

Checker output:
$err_snip

Stop only when that checker exits 0." || turn_ec=$?
        ec=$turn_ec
      done
      if [ "$v" -ge "$MAX_VERIFY" ]; then
        echo "{\"type\":\"harness_verify\",\"status\":\"exhausted\",\"attempts\":$MAX_VERIFY}"
        ec=1
      fi
    fi
    exit "$ec"
    ;;
  *) echo "unsupported BENCH_CLI=$CLI" >&2; exit 2 ;;
esac

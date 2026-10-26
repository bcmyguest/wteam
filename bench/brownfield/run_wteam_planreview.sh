#!/bin/sh
# wteam + plan-review (EGM Goldfish) on the brownfield task.
# Usage: sh run_wteam_planreview.sh <output-directory> <log-directory> [pairs]
set -eu

OUT=$1
LOGS=$2
PAIRS=${3:-2}
HERE=$(cd "$(dirname "$0")" && pwd)
MODEL=${WTEAM_BENCH_MODEL:-composer-2.5}
WORKER_MODEL=${WTEAM_BENCH_WORKER_MODEL:-$MODEL}
REVIEWER_MODEL=${WTEAM_BENCH_REVIEWER_MODEL:-$MODEL}
# Critic/readiness above comprehension (matches built-in tiers).
SPEC_LIGHT=${WTEAM_BENCH_SPEC_LIGHT_MODEL:-$MODEL}
SPEC_HEAVY=${WTEAM_BENCH_SPEC_HEAVY_MODEL:-$MODEL}
REWRITE_CAP=${WTEAM_BENCH_PLAN_REWRITE_CAP:-3}

sh "$HERE/prepare.sh" "$OUT" wteam
rm -rf "$LOGS"
mkdir -p "$LOGS"
mkdir -p "$OUT/.wteam/agents"
write_agent() {
  role=$1
  profile=$2
  role_model=$3
  printf 'name = "%s"\ncli = "cursor"\nmodel = "%s"\nprompt = "%s"\nprofile = "%s"\nextra_args = ["--force"]\n' \
    "$role" "$role_model" "$role" "$profile" >"$OUT/.wteam/agents/$role.toml"
}
write_agent planner plan "$MODEL"
write_agent orchestrator control "$MODEL"
write_agent worker edit "$WORKER_MODEL"
write_agent reviewer review "$REVIEWER_MODEL"
write_agent spec-comprehension plan "$SPEC_LIGHT"
write_agent spec-critic plan "$SPEC_HEAVY"
write_agent spec-readiness plan "$SPEC_HEAVY"

GOAL=$(cat "$HERE/task.md")
PLAN_PROMPT="$(wteam --root "$OUT" --slug bench role planner)

Goal:
$GOAL

Slug: bench
Write the spec and 2-6 implementation tickets under .wteam/bench/. Use the existing code and tests for Context and Verify. Finish only after wteam --slug bench pickup reports no problems."
START=$(date +%s)
agent -p --output-format stream-json --trust --force \
  --model "$MODEL" --workspace "$OUT" "$PLAN_PROMPT" \
  >"$LOGS/plan.jsonl" 2>"$LOGS/plan.stderr.log"
wteam --root "$OUT" --slug bench pickup >"$LOGS/pickup.txt"

# Goldfish: fresh-context personas; planner rewrite loops on FAIL (skill cap).
i=0
while true; do
  i=$((i + 1))
  wteam --root "$OUT" --slug bench restore >/dev/null 2>&1 || true
  if wteam --root "$OUT" --slug bench plan-review >"$LOGS/plan-review.$i.txt" 2>&1; then
    echo "plan_review_pass_attempt $i" >>"$LOGS/metrics.txt"
    break
  fi
  if [ "$i" -gt "$REWRITE_CAP" ]; then
    echo "plan_review_failed_after $i" >>"$LOGS/metrics.txt"
    echo "plan-review did not pass after $REWRITE_CAP rewrite loops" >&2
    END=$(date +%s)
    echo "wall_seconds $((END - START))" >>"$LOGS/metrics.txt"
    python3 "$HERE/token_usage.py" "$LOGS" "$OUT/.wteam/bench/logs" >>"$LOGS/metrics.txt"
    exit 1
  fi
  FINDINGS=$(cat "$LOGS/plan-review.$i.txt")
  REWRITE_PROMPT="$(wteam --root "$OUT" --slug bench role planner)

Slug: bench
Plan-review FAILED. Apply the **minimum** edit to clear the FINDINGS below.
Rules for this rewrite:
- Touch only tickets/spec lines the findings name.
- Do not invent new architecture, new tickets, or new Verify scenarios.
- Do not expand scope; prefer deleting a bad claim over adding a new one.
- Keep Context/Verify grounded in the existing repo.
Then run wteam --slug bench pickup until clean.

Findings:
$FINDINGS"
  agent -p --output-format stream-json --trust --force \
    --model "$MODEL" --workspace "$OUT" "$REWRITE_PROMPT" \
    >"$LOGS/rewrite.$i.jsonl" 2>"$LOGS/rewrite.$i.stderr.log"
  wteam --root "$OUT" --slug bench restore >/dev/null 2>&1 || true
  wteam --root "$OUT" --slug bench pickup >"$LOGS/pickup.after-rewrite.$i.txt" 2>&1 || true
done

wteam --root "$OUT" --slug bench run --pairs "$PAIRS" --dispatch auto --replan --timeout 1800 --max-turns 200 >"$LOGS/run.txt" 2>&1

# Backstop if the orchestrator skipped zoom-out validation.
write_agent validate-local review "$SPEC_LIGHT"
write_agent validate-mid review "$SPEC_HEAVY"
write_agent validate-out review "$SPEC_HEAVY"
vi=0
VALIDATION_OK=1
while true; do
  vi=$((vi + 1))
  wteam --root "$OUT" --slug bench restore >/dev/null 2>&1 || true
  if wteam --root "$OUT" --slug bench phase validate >"$LOGS/validate.$vi.txt" 2>&1; then
    echo "validate_pass_attempt $vi" >>"$LOGS/metrics.txt"
    break
  fi
  if [ "$vi" -gt "$REWRITE_CAP" ]; then
    echo "validate_failed_after $vi" >>"$LOGS/metrics.txt"
    VALIDATION_OK=0
    break
  fi
  # Orchestrator-shaped backstop: requeue is judgment; harness only retries once the phase fails.
  echo "validate attempt $vi failed; see $LOGS/validate.$vi.txt" >>"$LOGS/metrics.txt"
  VALIDATION_OK=0
  break
done

END=$(date +%s)
echo "wall_seconds $((END - START))" >>"$LOGS/metrics.txt"
python3 "$HERE/token_usage.py" "$LOGS" "$OUT/.wteam/bench/logs" >>"$LOGS/metrics.txt"
GRADE_OK=1
if sh "$HERE/grade.sh" "$OUT" >"$LOGS/grade.txt" 2>&1; then
  echo "grade_pass 1" >>"$LOGS/metrics.txt"
  tail -n 5 "$LOGS/grade.txt"
else
  echo "grade_pass 0" >>"$LOGS/metrics.txt"
  GRADE_OK=0
  tail -n 12 "$LOGS/grade.txt"
fi
if [ "$VALIDATION_OK" -ne 1 ]; then
  echo "validation_pass 0" >>"$LOGS/metrics.txt"
  echo "validation failed; see $LOGS/validate.$vi.txt" >&2
else
  echo "validation_pass 1" >>"$LOGS/metrics.txt"
fi
if [ "$GRADE_OK" -ne 1 ] || [ "$VALIDATION_OK" -ne 1 ]; then
  exit 1
fi

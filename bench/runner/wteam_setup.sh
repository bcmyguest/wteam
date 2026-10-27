# wteam agent TOMLs and optional codegraph (aligned with bench/commit0/run_wteam.sh).
# shellcheck shell=sh

bench_write_agent() {
  out=$1
  role=$2
  profile=$3
  model=$4
  cli=${5:-$(bench_cli)}
  mkdir -p "$out/.wteam/agents"
  extra=
  if [ "$cli" = cursor ]; then
    extra='extra_args = ["--force"]'
  fi
  {
    printf 'name = "%s"\ncli = "%s"\nmodel = "%s"\nprompt = "%s"\nprofile = "%s"\n' \
      "$role" "$cli" "$model" "$role" "$profile"
    if [ -n "$extra" ]; then
      printf '%s\n' "$extra"
    fi
  } >"$out/.wteam/agents/$role.toml"
  return 0
}

bench_setup_wteam_agents() {
  out=$1
  cg_prefix=${2:-}
  planner=$(bench_planner_model)
  worker=$(bench_worker_model)
  reviewer=$(bench_reviewer_model)
  cli=$(bench_cli)
  bench_write_agent "$out" planner "${cg_prefix}plan" "$planner" "$cli"
  bench_write_agent "$out" orchestrator "${cg_prefix}control" "$worker" "$cli"
  bench_write_agent "$out" worker "${cg_prefix}edit" "$worker" "$cli"
  review_mode=${BENCH_REVIEW:-autopass}
  case $review_mode in
    autopass|off)
      install_autopass_reviewer "$out"
      printf 'name = "reviewer"\ncli = "fake"\nmodel = "fake"\nprompt = "autopass"\nprofile = "%sreview"\n' \
        "$cg_prefix" >"$out/.wteam/agents/reviewer.toml"
      ;;
    on|real|cursor)
      if [ "$review_mode" = cursor ] || [ "$cli" = cursor ]; then
        write_reviewer_agent "$out" "$reviewer"
      else
        bench_write_agent "$out" reviewer "${cg_prefix}review" "$reviewer" "$cli"
      fi
      ;;
    *)
      echo "unknown BENCH_REVIEW=$review_mode (autopass|on|real|cursor)" >&2
      exit 2
      ;;
  esac
}

bench_codegraph_setup() {
  out=$1
  if [ "${CODEGRAPH:-0}" != 1 ]; then
    CG=
    export CG
    return 0
  fi
  mkdir -p "$out/.wteam/hooks" "$out/.wteam/profiles"
  printf '#!/bin/sh\ncodegraph init "$1" >/dev/null 2>&1 || codegraph sync "$1" >/dev/null 2>&1\n' \
    >"$out/.wteam/hooks/worktree"
  printf '%s\n' \
    'Code index: `codegraph explore "<symbol or file names>"` — use before long Read/grep chains.' \
    >"$out/.wteam/brief.md"
  for b in edit review plan control; do
    printf 'extends = "%s"\nallow = ["Bash(codegraph *)"]\n' "$b" >"$out/.wteam/profiles/cg-$b.toml"
  done
  CG=cg-
  export CG
  command -v codegraph >/dev/null 2>&1 && (cd "$out" && codegraph init . >/dev/null 2>&1) || true
  mkdir -p "$out/.git/info"
  grep -qF '.codegraph/' "$out/.git/info/exclude" 2>/dev/null || printf '.codegraph/\n' >>"$out/.git/info/exclude"
}

# Polyglot: tickets come from prepare.sh; orchestrator + worker + reviewer only.
bench_setup_polyglot_agents() {
  out=$1
  worker=$(bench_worker_model)
  reviewer=$(bench_reviewer_model)
  cli=$(bench_cli)
  bench_write_agent "$out" orchestrator control "$worker" "$cli"
  bench_write_agent "$out" worker edit "$worker" "$cli"
  review_mode=${BENCH_REVIEW:-autopass}
  case $review_mode in
    autopass|off)
      install_autopass_reviewer "$out"
      printf 'name = "reviewer"\ncli = "fake"\nmodel = "fake"\nprompt = "autopass"\nprofile = "review"\n' \
        >"$out/.wteam/agents/reviewer.toml"
      ;;
    on|real|cursor)
      if [ "$review_mode" = cursor ] || [ "$cli" = cursor ]; then
        write_reviewer_agent "$out" "$reviewer"
      else
        bench_write_agent "$out" reviewer review "$reviewer" "$cli"
      fi
      ;;
    *)
      echo "unknown BENCH_REVIEW=$review_mode (autopass|on|real|cursor)" >&2
      exit 2
      ;;
  esac
}

bench_plan_prompt() {
  goal=$1
  pairs=$2
  wteam=$3
  root=$4
  prompt="$("$wteam" --root "$root" --slug bench role planner)

Goal: $goal
Slug: bench
Write the spec to .wteam/bench/spec.md and one ticket per file to .wteam/bench/tickets/NN-<name>.md.
Group the work into 4-12 tickets by module, starting with a tracer bullet. The run has $pairs worker pairs.
Each ticket's Files line names the source files it edits. Omit Verify lines (the tests are hidden).
Finish only when \`wteam pickup --slug bench\` reports no problems."
  if [ "${SHAPE:-}" = coupled ]; then
    prompt="$prompt
Cut tickets along coupling: a class whose methods call each other belongs to one ticket, never sliced across several; a subclass may get its own ticket blocked by its base class. Fan out only across modules that meet through a few stable signatures.
The last ticket is an integration ticket, blocked by every other ticket: it exercises the whole public API end to end (the README and docs examples) and fixes the seams between tickets. Its Files line names every source file."
  fi
  if [ -f "$root/.wteam/brief.md" ]; then
    prompt="$prompt

$(cat "$root/.wteam/brief.md")"
  fi
  printf '%s\n' "$prompt"
}

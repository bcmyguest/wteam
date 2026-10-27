# Resumable matrix phases (source from run_matrix / overnight).
# shellcheck shell=sh

# Markers next to status.txt:
#   <name>.phase_done | .phase_failed | .phase_running | .phase_exit

phase_running() {
  base=$1
  name=$2
  mkdir -p "$base"
  date -Iseconds >"$base/$name.phase_running"
  rm -f "$base/$name.phase_done" "$base/$name.phase_failed" "$base/$name.phase_exit"
}

phase_finish() {
  base=$1
  name=$2
  ec=$3
  rm -f "$base/$name.phase_running"
  if [ "$ec" -eq 0 ]; then
    date -Iseconds >"$base/$name.phase_done"
    rm -f "$base/$name.phase_failed" "$base/$name.phase_exit"
  else
    date -Iseconds >"$base/$name.phase_failed"
    printf '%s\n' "$ec" >"$base/$name.phase_exit"
  fi
  echo "$(date -Iseconds) $name exit $ec" >>"$base/status.txt"
}

# RESUME=1: skip arms that finished successfully.
phase_should_run() {
  base=$1
  name=$2
  if [ "${RESUME:-0}" != 1 ]; then
    return 0
  fi
  if [ -f "$base/$name.phase_done" ]; then
    echo "skip $name (phase_done $(cat "$base/$name.phase_done" 2>/dev/null))"
    return 1
  fi
  if [ -f "$base/$name.phase_running" ]; then
    echo "retry $name (was running at $(cat "$base/$name.phase_running" 2>/dev/null))"
  elif [ -f "$base/$name.phase_failed" ]; then
    echo "retry $name (failed exit $(cat "$base/$name.phase_exit" 2>/dev/null))"
  fi
  return 0
}

init_matrix_status() {
  base=$1
  mkdir -p "$base"
  if [ ! -f "$base/status.txt" ]; then
    {
      echo "matrix_base $base"
      echo "started $(date -Iseconds)"
      shift
      for line; do
        printf '%s\n' "$line"
      done
    } >"$base/status.txt"
  fi
}

# Infer .phase_done from status.txt lines like "... harness-tb continuous exit 0"
bootstrap_phase_done_from_status() {
  base=$1
  prefix=$2
  [ -f "$base/status.txt" ] || return 0
  for mode in continuous fresh team; do
    name="${prefix}${mode}"
    [ -f "$base/$name.phase_done" ] && continue
    [ -f "$base/$name.phase_failed" ] && continue
    if grep -E "${mode} exit 0$" "$base/status.txt" >/dev/null 2>&1; then
      date -Iseconds >"$base/$name.phase_done"
      echo "bootstrapped $name.phase_done from status.txt"
    fi
  done
}

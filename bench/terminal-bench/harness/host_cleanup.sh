#!/bin/sh
# Between-task / post-job host cleanup + disk monitor for TB harness sweeps.
# Reclaims Harbor/TB Docker leftovers and old /tmp/wteam-* scratch without
# touching unrelated stacks or the active TB_HARNESS_DIR.
#
# Exit codes: 0 ok, 2 disk below TB_DISK_MIN_AVAIL_GB after cleanup.
set -eu

KEEP=${TB_HARNESS_DIR:-}
KEEP2=${TB_CLEANUP_KEEP:-}
MIN_GB=${TB_DISK_MIN_AVAIL_GB:-15}
WARN_GB=${TB_DISK_WARN_AVAIL_GB:-40}
LOG=
if [ -n "$KEEP" ] && [ -d "$KEEP" ]; then
  LOG=$KEEP/disk-monitor.log
fi

avail_gb() {
  # Portable-ish: df -Pk on /
  df -Pk / 2>/dev/null | awk 'NR==2 {printf "%d", $4/1024/1024}'
}

log() {
  line=$(printf '%s %s\n' "$(date -Iseconds 2>/dev/null || date)" "$*")
  printf '%s\n' "$line" >&2
  [ -n "$LOG" ] && printf '%s\n' "$line" >>"$LOG"
}

AVAIL=$(avail_gb || echo 0)
log "cleanup_start avail_gb=${AVAIL} min=${MIN_GB} warn=${WARN_GB}"

# Remove exited TB trial containers Harbor sometimes leaves behind.
docker ps -aq --filter status=exited --filter name='__env-' 2>/dev/null \
  | xargs -r docker rm -f >/dev/null 2>&1 || true
docker ps -aq --filter status=exited --filter name='__verifier' 2>/dev/null \
  | xargs -r docker rm -f >/dev/null 2>&1 || true

# Dangling layers only (safe on a shared host).
docker image prune -f >/dev/null 2>&1 || true

# Drop unused Terminal-Bench task images from prior trials (rmi no-ops if still in use).
# This is the main growth path during a 66-task sweep.
docker images --format '{{.Repository}}:{{.Tag}}' 2>/dev/null \
  | grep -E '^harborframework/terminal-bench' \
  | while read -r img; do
      docker rmi "$img" >/dev/null 2>&1 || true
    done

# Harbor compose temp dirs under /tmp (older than 30 min).
find /tmp -maxdepth 1 -type d \( -name 'tmp.*' -o -name 'tmp??????' \) -mmin +30 \
  -exec rm -rf {} + 2>/dev/null || true

# Old harness scratch (>2h), never the active job dir.
for d in /tmp/wteam-harness-tb-* /tmp/wteam-tb-*; do
  [ -e "$d" ] || continue
  [ -n "$KEEP" ] && [ "$d" = "$KEEP" ] && continue
  [ -n "$KEEP2" ] && [ "$d" = "$KEEP2" ] && continue
  find "$d" -maxdepth 0 -mmin +120 -exec rm -rf {} + 2>/dev/null || true
done

find /tmp -maxdepth 1 -type f \( -name 'opencode-harness*' -o -name 'lemonade-load-*' -o -name 'lemonade-unload-*' \) \
  -mmin +60 -delete 2>/dev/null || true

AVAIL=$(avail_gb || echo 0)
log "cleanup_end avail_gb=${AVAIL}"

if [ "$AVAIL" -lt "$MIN_GB" ]; then
  log "DISK_CRITICAL avail_gb=${AVAIL} < min=${MIN_GB}"
  exit 2
fi
if [ "$AVAIL" -lt "$WARN_GB" ]; then
  log "DISK_WARN avail_gb=${AVAIL} < warn=${WARN_GB}"
fi
exit 0

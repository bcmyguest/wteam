# Shared demo environment (source from play.sh / main.sh).
DEMO_ROOT="${WTEAM_DEMO_ROOT:-/tmp/wteam-demo}"
SCRIPT_DIR=$(CDPATH= cd -- "$(dirname "$0")" && pwd)
REPO=$(CDPATH= cd -- "$SCRIPT_DIR/../.." && pwd)
WTEAM_BIN="${WTEAM_BIN:-$REPO/target/release}"
export PATH="$WTEAM_BIN:${PATH}"
export WTEAM_FAKE_DIR="$DEMO_ROOT/.wteam-fake"
export WTEAM_DEMO_ROOT="$DEMO_ROOT"
export WTEAM_DEMO_PATH="$WTEAM_BIN:${PATH}"
export WTEAM_DEMO_SLUG=demo
export GIT_PAGER=cat
export PAGER=cat

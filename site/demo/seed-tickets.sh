#!/bin/sh
# Creates the greet-CLI ticket graph (printed commands show up in the demo GIF).
set -eu
export PATH="${WTEAM_DEMO_PATH:-/opt/wteam:$PATH}"
run() {
  printf '> %s\n' "$*"
  "$@"
}
run wteam --slug demo tickets add "Tracer bullet" --files src/core.py
run wteam --slug demo tickets add "Parser" --blocked-by 01 --files src/parser.py
run wteam --slug demo tickets add "Renderer" --blocked-by 01 --files src/render.py
run wteam --slug demo tickets add "CLI glue" --blocked-by 02,03 --files src/greet.py

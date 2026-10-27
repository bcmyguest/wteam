# Polyglot prompts (source after prepare.sh, cwd = work dir).
# shellcheck shell=sh

if [ "${HIDDEN:-0}" = 1 ]; then
  POLYGLOT_GOAL='Solve every exercise in this repository. Each top-level directory is one exercise with INSTRUCTIONS.md (the statement) and a stub <name>.py. Implement each stub exactly as its INSTRUCTIONS.md specifies, including every edge case and error it describes. A hidden test suite grades the result, so write and run your own checks with python3. Work through ALL directories before you finish.'
else
  POLYGLOT_GOAL='Solve every exercise in this repository. Each top-level directory is one exercise with INSTRUCTIONS.md (the statement), a stub <name>.py and <name>_test.py. Implement each stub so its tests pass; check with `python3 -m pytest -q <dir>`. Never edit test files. Work through ALL directories before you finish, then report how many pass.'
fi

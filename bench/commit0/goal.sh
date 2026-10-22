# Sourced by the runners: the shared task statement for a Commit0 rebuild.
# Expects $LIB.
PKG="$LIB"
GOAL="Rebuild the \`$PKG\` Python library in this repository. Every function and method whose body is a stub (\`pass\`) must be implemented so the library behaves as documented in spec.pdf, the README and docs/. Keep every public name, signature and module layout unchanged. The library's own test suite is hidden and will grade the result, so write and run your own checks under scratch/ (create files with the file-editing tool; run them with one-line python3 commands). Run exactly one command per shell call: no \`;\`, \`&&\`, \`|\` or multi-line commands."
if [ "$LIB" = incidentkit ]; then
GOAL="Implement the \`incidentkit\` Python package in this repository exactly as SPEC.md specifies: every module, name, signature, rule and error. Standard library only. A hidden test suite derived from SPEC.md will grade the result, so write and run your own checks under scratch/ (create files with the file-editing tool; run them with one-line python3 commands). Run exactly one command per shell call: no \`;\`, \`&&\`, \`|\` or multi-line commands."
fi
# Libraries with third-party deps: agents get the venv's python3/pytest first on PATH (sh venv.sh <lib>).
VENV="${BENCH_VENVS:-/tmp/commit0-venvs}/$LIB"
if [ -x "$VENV/bin/python3" ]; then PATH="$VENV/bin:$PATH"; export PATH; fi

#!/bin/sh
# Creates the per-lib venv that grade.sh uses (/tmp/commit0-venvs/<lib>): pytest + the lib's test/runtime deps.
#   sh venv.sh <lib>      libs: bitstring marshmallow imapclient
set -eu
LIB="$1"
V="${BENCH_VENVS:-/tmp/commit0-venvs}"
case "$LIB" in
  bitstring) DEPS="pytest hypothesis gfloat>=0.2.1,<0.3 pytest-benchmark bitarray>=2.9.0,<3.0.0" ;;
  marshmallow) DEPS="pytest pytz simplejson packaging" ;;
  imapclient) DEPS="pytest six" ;;
  *) echo "no venv needed or unknown lib $LIB" >&2; exit 2 ;;
esac
[ -x "$V/$LIB/bin/python" ] || python3 -m venv "$V/$LIB"
# shellcheck disable=SC2086
set -f; "$V/$LIB/bin/pip" install -q $DEPS; set +f
echo "$V/$LIB ready"

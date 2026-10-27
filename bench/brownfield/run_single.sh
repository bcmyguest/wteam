#!/bin/sh
exec sh "$(cd "$(dirname "$0")" && pwd)/run_continuous.sh" "$@"

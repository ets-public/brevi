#!/bin/sh
# A CPython whose output is upper-cased, so a case expecting upper case passes under it and fails natively:
# the MCP server must call that a Brevi bug, not the model's.
# Behind it, the CPython 3.13 Brevi itself would find first (src/cpython.rs).
py=$(command -v python3.13 || echo python3)
if [ "$1" = "-c" ]; then exec "$py" "$@"; fi
"$py" "$@" | tr a-z A-Z

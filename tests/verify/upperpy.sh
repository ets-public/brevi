#!/bin/sh
# A CPython whose output is upper-cased, so a case expecting upper case passes under it and fails natively:
# the MCP server must call that a Brevi bug, not the model's.
if [ "$1" = "-c" ]; then exec python3 "$@"; fi
python3 "$@" | tr a-z A-Z

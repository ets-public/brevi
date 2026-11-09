#!/bin/sh
# A CPython that takes 3 s to start, so `brevi run --verify` gives the native answer marked unverified.
if [ "$1" = "-c" ]; then exec python3 "$@"; fi
sleep 3
python3 "$@"

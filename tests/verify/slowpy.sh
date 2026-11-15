#!/bin/sh
# A CPython that takes 3 s to start, so `brevi run --verify` gives the native answer marked unverified.
# Behind it, the CPython 3.13 Brevi itself would find first (src/cpython.rs).
py=$(command -v python3.13 || echo python3)
if [ "$1" = "-c" ]; then exec "$py" "$@"; fi
sleep 3
"$py" "$@"

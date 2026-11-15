#!/bin/sh
# A CPython that prints one line more than python3, so verify has a wrong answer to find.
# Behind it, the CPython 3.13 Brevi itself would find first (src/cpython.rs).
py=$(command -v python3.13 || echo python3)
if [ "$1" = "-c" ]; then exec "$py" "$@"; fi
"$py" "$@"
echo "one more line"

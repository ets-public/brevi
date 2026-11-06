#!/bin/sh
# A CPython that prints one line more than python3, so verify has a wrong answer to find.
if [ "$1" = "-c" ]; then exec python3 "$@"; fi
python3 "$@"
echo "one more line"

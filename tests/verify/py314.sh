#!/bin/sh
# A CPython that says it is 3.14, so Brevi must refuse it: Brevi answers as CPython 3.13 does (src/cpython.rs).
case "$2" in *sys.implementation*) echo "cpython 3.14.0 $0"; exit 0;; esac
exec python3 "$@"

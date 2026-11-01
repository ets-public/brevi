#!/bin/sh
# The headless tkinter stand-in (bench/tkshim, for CPython) and Brevi's GUI runtime (runtime/brevi_tk) must
# print the same for the same program and actions: runs examples/counter.py and examples/counter.rs. From the repo root.
set -e
S='show\nclick +1\nclick +1\ntype 1 Adam\nkey 1 Return\nselect 1 2\nshow\nwait 999\nshow\nwait 1\nshow\nclick nope\n'
(cd runtime/brevi_tk && cargo build -q --example counter)
a=$(printf "$S" | PYTHONPATH=bench/tkshim python3 runtime/brevi_tk/examples/counter.py)
b=$(printf "$S" | BREVI_GUI=headless runtime/brevi_tk/target/debug/examples/counter)
[ "$a" = "$b" ] || { echo "tkshim and brevi_tk differ:"; printf '%s\n' "$a" > /tmp/gui_a; printf '%s\n' "$b" | diff /tmp/gui_a -; exit 1; }
echo "tkshim and brevi_tk agree"

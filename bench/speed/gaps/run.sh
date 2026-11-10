#!/bin/sh
# Build each benchmark with Brevi (--release) and run it next to CPython.
# Usage: sh bench/speed/gaps/run.sh [NAME...]   PY=python3 BREVI=target/release/brevi
cd "$(dirname "$0")"
PY=${PY:-python3}
BREVI=${BREVI:-../../../target/release/brevi}
OUT=${OUT:-/tmp/brevi-gaps}
mkdir -p "$OUT"
names=${*:-"methods jinja_chat pyd_bulk polars_big async_loop sqlite_point"}
for n in $names; do
  "$BREVI" build "$n.py" --release -o "$OUT/$n" >"$OUT/$n.build" 2>&1 || { echo "$n: build failed"; tail -5 "$OUT/$n.build"; continue; }
  "$PY" "$n.py" >"$OUT/$n.py.out" 2>"$OUT/$n.py.err"
  "$OUT/$n" >"$OUT/$n.bv.out" 2>"$OUT/$n.bv.err"
  same=same; cmp -s "$OUT/$n.py.out" "$OUT/$n.bv.out" || same=DIFFERENT
  printf '%-14s cpython: %-40s brevi: %-40s output %s\n' "$n" "$(grep time "$OUT/$n.py.err")" "$(grep time "$OUT/$n.bv.err")" "$same"
done

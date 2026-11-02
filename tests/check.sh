#!/bin/sh
# Runs every program in tests/run and every error case in tests/errors,
# and every .py file in tests/fallback, comparing output with the .expected file next to it. Run from the repo root.
# Pass --update to rewrite the expected files.
BREVI=${BREVI:-./target/debug/brevi}
fail=0
for f in tests/run/*.bv tests/errors/*.bv; do
  exp="${f%.bv}.expected"
  case "$f" in
    tests/run/*) inp="${f%.bv}.in"; [ -f "$inp" ] || inp=/dev/null; out=$($BREVI run "$f" < "$inp" 2>&1; $BREVI test "$f" 2>&1 | grep "test result" | sed "s/; finished in .*//") ;;
    *) out=$(cd tests/errors && ../../$BREVI check "$(basename "$f")" 2>&1) ;;
  esac
  if [ "$1" = "--update" ]; then printf '%s\n' "$out" > "$exp"; continue; fi
  if [ "$out" != "$(cat "$exp")" ]; then
    echo "FAIL $f"; printf '%s\n' "$out" | diff "$exp" - ; fail=1
  else
    echo "ok   $f"
  fi
done
# brevi run on a .py file: runs with python3 when Brevi cannot build it (with a note), unless --native.
for f in tests/fallback/*.py; do
  exp="${f%.py}.expected"
  out=$($BREVI run "$f" 2>&1; $BREVI run "$f" --native >/dev/null 2>&1; echo "native exit $?")
  if [ "$1" = "--update" ]; then printf '%s\n' "$out" > "$exp"; continue; fi
  if [ "$out" != "$(cat "$exp")" ]; then
    echo "FAIL $f"; printf '%s\n' "$out" | diff "$exp" - ; fail=1
  else
    echo "ok   $f"
  fi
done
exit $fail

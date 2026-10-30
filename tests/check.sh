#!/bin/sh
# Runs every program in tests/run and every error case in tests/errors,
# comparing output with the .expected file next to it. Run from the repo root.
# Pass --update to rewrite the expected files.
AXON=${AXON:-./target/debug/axon}
fail=0
for f in tests/run/*.ax tests/errors/*.ax; do
  exp="${f%.ax}.expected"
  case "$f" in
    tests/run/*) inp="${f%.ax}.in"; [ -f "$inp" ] || inp=/dev/null; out=$($AXON run "$f" < "$inp" 2>&1; $AXON test "$f" 2>&1 | grep "test result" | sed "s/; finished in .*//") ;;
    *) out=$(cd tests/errors && ../../$AXON check "$(basename "$f")" 2>&1) ;;
  esac
  if [ "$1" = "--update" ]; then printf '%s\n' "$out" > "$exp"; continue; fi
  if [ "$out" != "$(cat "$exp")" ]; then
    echo "FAIL $f"; printf '%s\n' "$out" | diff "$exp" - ; fail=1
  else
    echo "ok   $f"
  fi
done
exit $fail

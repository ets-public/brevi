#!/bin/sh
# Runs every program in tests/run and every error case in tests/errors,
# every .py file in tests/fallback, every .py file in tests/typed (built typed), and every .py file in tests/dyn (built over dynamic values), comparing output with the .expected file next to it. Run from the repo root.
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
# Python mistakes CPython would also stop on: check and run report them (exit 1) instead of building the file
# over dynamic values; py_valid.py is the control (a caught TypeError, a function defined below its caller).
for f in tests/errors/*.py; do
  exp="${f%.py}.expected"
  out=$(cd tests/errors && b=$(basename "$f") && ../../$BREVI check "$b" 2>&1; echo "check exit $?"; ../../$BREVI run "$b" 2>&1; echo "run exit $?")
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
# Python programs built typed (BREVI_DYN=0 --native) must print what python3 prints, in time: the set
# tests take minutes if `in` scans a list instead of using AxSet's hash index.
for f in tests/typed/*.py; do
  exp="${f%.py}.expected"
  out=$(BREVI_DYN=0 timeout 60 $BREVI run "$f" --native 2>&1)
  if [ "$1" = "--update" ]; then python3 "$f" > "$exp" 2>&1; continue; fi
  if [ "$out" != "$(cat "$exp")" ]; then
    echo "FAIL $f"; printf '%s\n' "$out" | diff "$exp" - ; fail=1
  else
    echo "ok   $f"
  fi
done
# Python programs built over dynamic values (BREVI_DYN=1 skips the typed build) must print what python3 prints.
for f in tests/dyn/*.py; do
  exp="${f%.py}.expected"
  out=$(BREVI_DYN=1 $BREVI run "$f" 2>&1)
  if [ "$1" = "--update" ]; then python3 "$f" > "$exp" 2>&1; continue; fi
  if [ "$out" != "$(cat "$exp")" ]; then
    echo "FAIL $f"; printf '%s\n' "$out" | diff "$exp" - ; fail=1
  else
    echo "ok   $f"
  fi
done
# brevi run --sandbox on a .py file: a capability refusal is final on every path (the default run, dynamic
# builds, typed --native), and nothing falls back to python3 or reaches CPython through a library.
for f in tests/sandbox/*.py; do
  exp="${f%.py}.expected"
  out=$($BREVI run --sandbox "$f" 2>&1; echo "exit $?"; echo "-- dynamic"; BREVI_DYN=1 $BREVI run --sandbox "$f" 2>&1; echo "exit $?"; echo "-- typed"; BREVI_DYN=0 $BREVI run --sandbox --native "$f" 2>&1; echo "exit $?")
  if [ "$1" = "--update" ]; then printf '%s\n' "$out" > "$exp"; continue; fi
  if [ "$out" != "$(cat "$exp")" ]; then
    echo "FAIL $f"; printf '%s\n' "$out" | diff "$exp" - ; fail=1
  else
    echo "ok   $f"
  fi
done
# brevi build --standalone: a program calling Python libraries carries its own CPython. Run without python3 on
# PATH and, where user namespaces allow it, in an empty root holding only the C library.
for f in tests/standalone/*.py; do
  exp="${f%.py}.expected"
  t=$(mktemp -d)
  $BREVI build "$f" --native --standalone -o "$t/prog" >/dev/null 2>&1
  out=$(env -i HOME="$t" PATH=/nonexistent BREVI_PY_CACHE="$t/cache" "$t/prog" 2>&1; ls "$t"/cache/*/.brevi-ok >/dev/null 2>&1 || echo "bundle not unpacked")
  if unshare -r true 2>/dev/null; then
    mkdir -p "$t/root/tmp"; cp "$t/prog" "$t/root/prog"
    for l in $(ldd "$t/prog" | grep -o '/[^ ]*') $(ldconfig -p | grep -E '/(libm|libpthread|libdl|librt|libutil)\.so\.[0-9]+$' | grep -o '/[^ ]*$'); do mkdir -p "$t/root$(dirname "$l")"; cp -L "$l" "$t/root$l"; done
    bare=$(unshare -r env -i HOME=/tmp "$(command -v chroot)" "$t/root" /prog 2>&1)
    [ "$bare" = "$out" ] || out="$out
in an empty root: $bare"
  fi
  rm -rf "$t"
  if [ "$1" = "--update" ]; then python3 "$f" > "$exp" 2>&1; continue; fi
  if [ "$out" != "$(cat "$exp")" ]; then
    echo "FAIL $f"; printf '%s\n' "$out" | diff "$exp" - ; fail=1
  else
    echo "ok   $f"
  fi
done
exit $fail

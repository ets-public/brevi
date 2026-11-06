#!/bin/sh
# Runs every program in tests/run and every error case in tests/errors,
# every .py file in tests/fallback, every .py file in tests/typed (built typed), and every .py file in tests/dyn (built over dynamic values), comparing output with the .expected file next to it; tests/exit also compares exit statuses. Run from the repo root.
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
# Python mistakes CPython would also stop on: check reports them (exit 1) instead of building the file over
# dynamic values, and run says so in its note when it runs the file with python3, then shows only the error
# line of CPython's traceback (error lines left out: they change between Python versions). py_valid.py and py_unreached.py are controls (a caught TypeError, a
# function defined below its caller, mistakes in branches that never run).
for f in tests/errors/*.py; do
  exp="${f%.py}.expected"
  out=$(cd tests/errors && b=$(basename "$f") && ../../$BREVI check "$b" 2>&1; echo "check exit $?"; r=$(../../$BREVI run "$b" 2>&1); c=$?; printf '%s\n' "$r" | grep -v '^  \|^Traceback\|^[A-Za-z]*Error'; echo "run exit $c"; echo "run tracebacks $(printf '%s\n' "$r" | grep -c '^Traceback')")
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
# Which path a Python file took ({"path":...} on stderr): check, run (stderr only) and run --explain.
# Files starting with _ are modules the tests import.
for f in tests/path/py_*.py; do
  exp="${f%.py}.expected"
  out=$(cd tests/path && b=$(basename "$f") && ../../$BREVI check "$b" 2>&1; ../../$BREVI run "$b" 2>&1 >/dev/null; ../../$BREVI run "$b" --explain 2>&1 >/dev/null)
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
# Standard-library programs must print what python3 prints, natively: whichever path the build takes
# (typed or dynamic), never CPython (pybridge or the python3 fallback).
for f in tests/lib/*.py; do
  exp="${f%.py}.expected"
  out=$(BREVI_PATH=1 timeout 120 $BREVI run "$f" --native 2>/tmp/brevi_lib_err)
  if [ "$1" = "--update" ]; then python3 "$f" > "$exp" 2>&1; continue; fi
  if grep -q '"path":"\(pybridge\|cpython\)"' /tmp/brevi_lib_err; then
    echo "FAIL $f (ran in CPython)"; cat /tmp/brevi_lib_err; fail=1
  elif [ "$out" != "$(cat "$exp")" ]; then
    echo "FAIL $f"; cat /tmp/brevi_lib_err; printf '%s\n' "$out" | diff "$exp" - ; fail=1
  else
    echo "ok   $f"
  fi
done
# numpy programs must print what CPython's numpy prints, byte for byte, with Brevi's own numpy: never CPython.
# The .expected files come from real numpy (numpy 2.x; CI has none): `--update` needs it installed.
# A test that prints full precision can differ between numpy's AVX-512 and AVX2 kernels: it keeps the AVX2 output
# in NAME.avx2.expected, which a CPU without AVX-512 checks against.
np_avx512=1
for fl in avx512f avx512dq avx512cd avx512bw avx512vl; do grep -qw "$fl" /proc/cpuinfo 2>/dev/null || np_avx512=0; done
for f in tests/numpy/*.py; do
  exp="${f%.py}.expected"
  if [ "$1" = "--update" ]; then
    python3 -c "import numpy" 2>/dev/null && python3 "$f" > "$exp" 2>/dev/null
    [ -f "${f%.py}.avx2.expected" ] && OPENBLAS_CORETYPE=Haswell python3 "$f" > "${f%.py}.avx2.expected" 2>/dev/null
    continue
  fi
  [ "$np_avx512" = 0 ] && [ -f "${f%.py}.avx2.expected" ] && exp="${f%.py}.avx2.expected"
  out=$(BREVI_PATH=1 timeout 120 $BREVI run "$f" --native 2>/tmp/brevi_np_err)
  if ! grep -q '"path":"dynamic"' /tmp/brevi_np_err; then
    echo "FAIL $f (not built with Brevi's numpy)"; cat /tmp/brevi_np_err; fail=1
  elif [ "$out" != "$(cat "$exp")" ]; then
    echo "FAIL $f"; printf '%s\n' "$out" | diff "$exp" - ; grep -v '"path"' /tmp/brevi_np_err | tail -5; fail=1
  else
    echo "ok   $f"
  fi
done
# tiktoken programs must print what CPython's tiktoken prints, with Brevi's own tiktoken: never CPython. The
# .expected files come from tiktoken 0.14 (CI has none): `--update` needs it installed. Building one gets OpenAI's
# vocabularies once, into tiktoken's cache, as tiktoken itself does.
for f in tests/tiktoken/*.py; do
  exp="${f%.py}.expected"
  if [ "$1" = "--update" ]; then
    python3 -c "import tiktoken" 2>/dev/null && python3 "$f" > "$exp" 2>&1
    continue
  fi
  out=$(BREVI_PATH=1 timeout 300 $BREVI run "$f" --native 2>/tmp/brevi_tok_err)
  if ! grep -q '"path":"dynamic"' /tmp/brevi_tok_err; then
    echo "FAIL $f (not built with Brevi's tiktoken)"; cat /tmp/brevi_tok_err; fail=1
  elif [ "$out" != "$(cat "$exp")" ]; then
    echo "FAIL $f"; printf '%s\n' "$out" | diff "$exp" - ; grep -v '"path"' /tmp/brevi_tok_err | tail -5; fail=1
  else
    echo "ok   $f"
  fi
done
# tokenizers programs must print what CPython's tokenizers prints, with Brevi's own tokenizers: never CPython.
# The .expected files come from tokenizers 0.23.3 (CI has none): `--update` needs it installed. Building one
# gets each Hub tokenizer.json once (from huggingface_hub's cache when it has it, else into Brevi's cache).
for f in tests/tokenizers/*.py; do
  exp="${f%.py}.expected"
  if [ "$1" = "--update" ]; then
    python3 -c "import tokenizers" 2>/dev/null && python3 "$f" > "$exp" 2>/dev/null
    continue
  fi
  out=$(BREVI_PATH=1 timeout 300 $BREVI run "$f" --native 2>/tmp/brevi_hf_err)
  if ! grep -q '"path":"dynamic"' /tmp/brevi_hf_err; then
    echo "FAIL $f (not built with Brevi's tokenizers)"; cat /tmp/brevi_hf_err; fail=1
  elif [ "$out" != "$(cat "$exp")" ]; then
    echo "FAIL $f"; printf '%s\n' "$out" | diff "$exp" - ; grep -v '"path"' /tmp/brevi_hf_err | tail -5; fail=1
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
# Python libraries the dynamic build lacks (tests/bridge/brlib stands in for a third-party one) run in CPython
# inside a dynamic build: output must match python3, the path line must name them, and nothing may run fully
# in CPython. Each runs by default and with BREVI_DYN=1.
for f in tests/bridge/*.py; do
  exp="${f%.py}.expected"
  if [ "$1" = "--update" ]; then (cd tests/bridge && python3 "$(basename "$f")") > "$exp" 2>&1; continue; fi
  for mode in default dyn; do
    if [ $mode = dyn ]; then dv=1; else dv=; fi
    out=$(cd tests/bridge && BREVI_DYN=$dv BREVI_PATH=1 timeout 120 ../../$BREVI run "$(basename "$f")" 2>/tmp/brevi_bridge_err)
    if grep -q '"path":"cpython"' /tmp/brevi_bridge_err || ! grep -q '"py":\[' /tmp/brevi_bridge_err; then
      echo "FAIL $f ($mode: not bridged)"; cat /tmp/brevi_bridge_err; fail=1
    elif [ "$out" != "$(cat "$exp")" ]; then
      echo "FAIL $f ($mode)"; cat /tmp/brevi_bridge_err; printf '%s\n' "$out" | diff "$exp" - ; fail=1
    else
      echo "ok   $f ($mode)"
    fi
  done
done
# Under the C locale (no LANG), text files a bridged library reads decode as UTF-8, as in python3 (#211).
f=tests/bridge/c_locale/read_utf8.py
out=$(env -u LANG -u LC_ALL -u LC_CTYPE $BREVI run --native "$f" 2>/dev/null)
if [ "$out" != "Mio 🖤" ]; then
  echo "FAIL $f"; echo "$out"; fail=1
else
  echo "ok   $f"
fi
# brevi run --sandbox on a .py file: a capability refusal is final on every path (the default run, dynamic
# builds, typed --native), and nothing falls back to python3 or reaches CPython through a library.
# A .flags file next to a test holds what the caller grants (--allow read): the file's own allow lines grant nothing.
for f in tests/sandbox/*.py; do
  exp="${f%.py}.expected"
  g=$(cat "${f%.py}.flags" 2>/dev/null)
  out=$($BREVI run --sandbox $g "$f" 2>&1; echo "exit $?"; echo "-- dynamic"; BREVI_DYN=1 $BREVI run --sandbox $g "$f" 2>&1; echo "exit $?"; echo "-- typed"; BREVI_DYN=0 $BREVI run --sandbox $g --native "$f" 2>&1; echo "exit $?")
  if [ "$1" = "--update" ]; then printf '%s\n' "$out" > "$exp"; continue; fi
  if [ "$out" != "$(cat "$exp")" ]; then
    echo "FAIL $f"; printf '%s\n' "$out" | diff "$exp" - ; fail=1
  else
    echo "ok   $f"
  fi
done
# Exit status, which the loops above do not compare: a Python file must exit as python3 does on both native
# paths, with its stdout written first. A .bv program stops with status 101 at a runtime error.
for f in tests/exit/*.py; do
  exp="${f%.py}.expected"
  out=$(BREVI_DYN=0 $BREVI run "$f" --native 2>/dev/null; echo " typed exit $?"; BREVI_DYN=1 $BREVI run "$f" --native 2>/dev/null; echo " dynamic exit $?")
  if [ "$1" = "--update" ]; then (python3 "$f" 2>/dev/null; echo " typed exit $?"; python3 "$f" 2>/dev/null; echo " dynamic exit $?") > "$exp"; continue; fi
  if [ "$out" != "$(cat "$exp")" ]; then
    echo "FAIL $f"; printf '%s\n' "$out" | diff "$exp" - ; fail=1
  else
    echo "ok   $f"
  fi
done
for f in tests/exit/*.bv; do
  exp="${f%.bv}.expected"
  out=$($BREVI run "$f" 2>/dev/null; echo " exit $?")
  if [ "$1" = "--update" ]; then printf '%s\n' "$out" > "$exp"; continue; fi
  if [ "$out" != "$(cat "$exp")" ]; then
    echo "FAIL $f"; printf '%s\n' "$out" | diff "$exp" - ; fail=1
  else
    echo "ok   $f"
  fi
done
# A Python library under --sandbox: refused unless the caller names it with --allow-py (a `# allow native` line in
# the file grants nothing), and with it the program runs as it does unsandboxed.
B=$(cd "$(dirname "$BREVI")" && pwd)/$(basename "$BREVI")
want="$(cat tests/bridge/lib_values.expected)
exit 0"
for g in "" "--allow-py other" "--allow-py brlib"; do
  out=$(cd tests/bridge && $B run --sandbox $g lib_values.py 2>/dev/null; echo "exit $?")
  case "$g" in
    *brlib) ok=$([ "$out" = "$want" ] && echo y) ;;
    *) ok=$(printf '%s\n' "$out" | grep -qx 'exit 1' && printf '%s\n' "$out" | grep -q 'needs allow native' && ! printf '%s\n' "$out" | grep -q '^3 True' && echo y) ;;
  esac
  if [ "$ok" = y ]; then echo "ok   sandbox bridge (${g:-no grant})"; else echo "FAIL sandbox bridge (${g:-no grant})"; printf '%s\n' "$out"; fail=1; fi
done
t=$(mktemp -d); cp -r tests/bridge/brlib tests/bridge/lib_values.py "$t"; sed -i '1i # allow native' "$t/lib_values.py"
out=$(cd "$t" && $B run --sandbox lib_values.py 2>&1; echo "exit $?")
if printf '%s\n' "$out" | grep -q 'grants nothing under --sandbox' && printf '%s\n' "$out" | grep -qx 'exit 1'; then echo "ok   sandbox ignores # allow native"; else echo "FAIL sandbox ignores # allow native"; printf '%s\n' "$out"; fail=1; fi
rm -rf "$t"
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
# The result cache: an unchanged program is replayed (path report line included), a changed module is built
# again, and --no-cache always builds.
t=$(mktemp -d)
printf 'import helper\nprint(helper.twice(21))\n' > "$t/main.py"
printf 'def twice(n):\n    return n * 2\n' > "$t/helper.py"
r() { echo "-- $*"; env BREVI_CACHE="$t/cache" BREVI_TIME=1 BREVI_PATH=1 $BREVI "$@" "$t/main.py" 2>&1 | grep -v ' ms  [^r]' | sed 's/^time .* ms  /time: /'; }
out=$(r run; r run; r check; r check; r run --native
  printf 'def twice(n):\n    return n + n + 1\n' > "$t/helper.py"
  r run; r run --no-cache
  printf 'print(undefined_name)\n' > "$t/main.py"
  r check; r check)
rm -rf "$t"
# A build that failed because of the machine is never replayed: with the dynamic runtime library unable to build
# (here its cache folder is a file; in v0.24.6's release run, many cold builds at once), the file is refused with
# the typed build's reason, which must not be kept and shown again once the machine is fine.
t2=$(mktemp -d); mkdir "$t2/cache"; : > "$t2/cache/dynrt"
printf 'import argparse\ndef positive(s):\n    return int(s)\np = argparse.ArgumentParser()\np.add_argument("--n", type=positive)\nprint(p.parse_args(["--n", "3"]).n)\n' > "$t2/main.py"
t2out=$(for c in "run --native" "run --native" check check; do env BREVI_CACHE="$t2/cache" BREVI_TIME=1 $BREVI $c "$t2/main.py" 2>&1; echo "exit $?"; done)
if [ -n "$(ls -A "$t2/cache/runs" 2>/dev/null)" ] || printf '%s\n' "$t2out" | grep -q 'replayed\|brevi_cache' || [ "$(printf '%s\n' "$t2out" | grep -cx 'exit 1')" != 4 ]; then
  echo "FAIL result cache keeps a machine failure"; printf '%s\n' "$t2out"; ls -A "$t2/cache/runs"; fail=1
else echo "ok   result cache keeps no machine failure"; fi
rm -rf "$t2"
exp=tests/cache.expected
if [ "$1" = "--update" ]; then printf '%s\n' "$out" > "$exp"
elif [ "$out" != "$(cat "$exp")" ]; then echo "FAIL result cache"; printf '%s\n' "$out" | diff "$exp" - ; fail=1
else echo "ok   result cache"; fi
exit $fail

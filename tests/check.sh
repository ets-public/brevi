#!/bin/sh
# Runs every program in tests/run and every error case in tests/errors,
# every .py file in tests/fallback, every .py file in tests/typed (built typed), and every .py file in tests/dyn (built over dynamic values), comparing output (fed NAME.in on stdin when there is one) with the .expected file next to it; tests/exit also compares exit statuses. Run from the repo root.
# Pass --update to rewrite the expected files.
BREVI=${BREVI:-./target/debug/brevi}
fail=0
# has MODULES: python3 can import them (the sections for Brevi's copies of duckdb, onnxruntime and the mcp client
# need the real package's engine or client). With CHECK_REQUIRE_LIBS=1 (releases and the daily run, which install
# tests/requirements-native.txt) a missing one is a failure, never a skip.
has() {
  python3 -c "import $1" 2>/dev/null && return 0
  if [ "$CHECK_REQUIRE_LIBS" = 1 ]; then echo "FAIL python3 cannot import $1 (CHECK_REQUIRE_LIBS=1; pip install -r tests/requirements-native.txt)"; fail=1; fi
  return 1
}
#== run split
# tests/run and tests/errors check what the typed build makes of each file, Python-style ones included: a .bv
# file in plain Python is kept off the .py path here (BREVI_BV_PY=0, BREVI_BV_DYN=0; tests/bv_py checks that path).
for f in tests/run/*.bv tests/errors/*.bv; do
  export BREVI_BV_PY=0 BREVI_BV_DYN=0
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
unset BREVI_BV_PY BREVI_BV_DYN
#== errors_py split
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
#== fallback split
no_rustc_words='s/\(Rust did not compile\) ([^"]*)/\1 (rustc: ...)/'
# brevi run on a .py file: runs with python3 when Brevi cannot build it (with a note), unless --native.
for f in tests/fallback/*.py; do
  exp="${f%.py}.expected"
  out=$($BREVI run "$f" 2>&1; $BREVI run "$f" --native >/dev/null 2>&1; echo "native exit $?")
  if [ "$1" = "--update" ]; then printf '%s\n' "$out" > "$exp"; continue; fi
  # The note quotes rustc's first error, whose wording (how it prints a type, e.g. `Vec<_>` or `Vec<Vec<i64>>`)
  # is rustc's, not Brevi's: compare everything but that quote, so the test holds on any rustc.
  out=$(printf '%s\n' "$out" | sed "$no_rustc_words")
  want=$(sed "$no_rustc_words" "$exp")
  if [ "$out" != "$want" ]; then
    w=$(mktemp); printf '%s\n' "$want" > "$w"
    echo "FAIL $f"; printf '%s\n' "$out" | diff "$w" - ; rm -f "$w"; fail=1
  else
    echo "ok   $f"
  fi
done
# A class built on a builtin iterator type (zip, map, filter, an itertools type) builds over dynamic values and
# prints what python3 prints: directly, and through a base chosen at run time from such types used as values.
if [ "$1" != "--update" ]; then
  it_dir=$(mktemp -d)
  printf 'from itertools import chain\nclass C(chain):\n    pass\nprint(list(C([1], [2])), type(C([])).__name__)\n' > "$it_dir/a.py"
  printf 'from itertools import repeat\ndef f():\n    for cls in (repeat, zip):\n        class S(cls):\n            pass\n        print(S.__name__, S.__mro__[1].__name__, issubclass(S, cls))\nf()\n' > "$it_dir/b.py"
  for p in a b; do
    out=$(BREVI_DYN=1 $BREVI build "$it_dir/$p.py" -o "$it_dir/prog" 2>&1 && "$it_dir/prog" 2>&1)
    if [ "$out" = "$(python3 "$it_dir/$p.py" 2>&1)" ]; then
      echo "ok   dynamic build subclasses an iterator type ($p)"
    else
      echo "FAIL dynamic build subclasses an iterator type ($p)"; printf '%s\n' "$out" | tail -3; fail=1
    fi
  done
  rm -rf "$it_dir"
fi
# A warning whose line the dynamic build does not know is refused, never placed wrong: stacklevel above 1 when
# it builds, a call opening on a later line of its statement when it runs.
if [ "$1" != "--update" ]; then
  wr_dir=$(mktemp -d)
  out=$(BREVI_DYN=1 $BREVI build tests/warnings/stacklevel.py -o "$wr_dir/prog" 2>&1)
  case "$out" in
    *'warnings.warn with a stacklevel above 1 is not supported in dynamic builds'*) echo "ok   dynamic build refuses warnings.warn(stacklevel=2)" ;;
    *) echo "FAIL dynamic build refuses warnings.warn(stacklevel=2)"; printf '%s\n' "$out" | tail -3; fail=1 ;;
  esac
  out=$(BREVI_DYN=1 $BREVI build tests/warnings/later_line.py -o "$wr_dir/prog" 2>&1 && "$wr_dir/prog" 2>&1)
  case "$out" in
    *'a warning at a line of the program the dynamic build does not know is not supported in dynamic builds'*) echo "ok   a warning on a later line of its statement is refused" ;;
    *) echo "FAIL a warning on a later line of its statement is refused"; printf '%s\n' "$out" | tail -3; fail=1 ;;
  esac
  rm -rf "$wr_dir"
fi
#== path split
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
#== typed split
# Python programs built typed (BREVI_DYN=0 --native) must print what python3 prints, in time: the set
# tests take minutes if `in` scans a list instead of using AxSet's hash index.
for f in tests/typed/*.py; do
  exp="${f%.py}.expected"
  inp="${f%.py}.in"; [ -f "$inp" ] || inp=/dev/null
  out=$(BREVI_DYN=0 timeout 60 $BREVI run "$f" --native < "$inp" 2>&1)
  if [ "$1" = "--update" ]; then python3 "$f" < "$inp" > "$exp" 2>&1; continue; fi
  if [ "$out" != "$(cat "$exp")" ]; then
    echo "FAIL $f"; printf '%s\n' "$out" | diff "$exp" - ; fail=1
  else
    echo "ok   $f"
  fi
done
#== replay split
# An int past 64 bits on the typed build: `brevi run` runs the program again on the dynamic build (src/replay.rs),
# with the same stdin, passing on only the output not written yet; stdout and the exit status must be python3's.
# no_*.py may not run twice (they write a file): they stop with an error (their .expected is Brevi's own).
for f in tests/replay/*.py; do
  exp="${f%.py}.expected"
  inp="${f%.py}.in"; [ -f "$inp" ] || inp=/dev/null
  out=$(timeout 120 $BREVI run "$f" --native < "$inp" 2>/dev/null; echo "exit $?")
  if [ "$1" = "--update" ]; then
    case "$f" in */no_*) printf '%s\n' "$out" > "$exp" ;; *) (python3 "$f" < "$inp" 2>/dev/null; echo "exit $?") > "$exp" ;; esac
    continue
  fi
  if [ "$out" != "$(cat "$exp")" ]; then
    echo "FAIL $f"; printf '%s\n' "$out" | diff "$exp" - ; fail=1
  else
    echo "ok   $f"
  fi
done
# More piped input than one kept piece (8 MB), with the int passing 64 bits only after all of it was read.
big=$(seq 1 2000000 | timeout 120 $BREVI run tests/replay/late_overflow.py --native 2>/dev/null; echo "exit $?")
want=$(seq 1 2000000 | python3 tests/replay/late_overflow.py; echo "exit $?")
if [ "$big" != "$want" ]; then echo "FAIL late_overflow (piped)"; fail=1; else echo "ok   late_overflow (piped)"; fi
#== strict
# --strict (BREVI_STRICT=1): no fallback. A file only the dynamic build takes is refused with one
# {"e":"strict"} line, a typed run past 64 bits stops instead of running again, brevi-python hands nothing to
# CPython, and every build prints its path line (typed too).
strict_out=$(cd tests/strict && B=../../$BREVI
  for f in typed guard cpython_only pybridge big_power; do
    echo "-- run $f"; $B run $f.py --strict 2>&1; echo "exit $?"
  done
  echo "-- check guard"; BREVI_STRICT=1 $B check guard.py 2>&1; echo "exit $?"
  echo "-- verify guard"; BREVI_STRICT=1 $B run --verify guard.py </dev/null 2>&1; echo "exit $?"
  ln -sf "$(cd ../.. && pwd)/$BREVI" ./brevi-python
  echo "-- shim -m"; BREVI_STRICT=1 ./brevi-python -m json.tool </dev/null 2>&1; echo "exit $?"
  echo "-- shim guard"; BREVI_STRICT=1 ./brevi-python guard.py 2>&1; echo "exit $?"
  echo "-- shim typed"; BREVI_STRICT=1 ./brevi-python typed.py 2>&1; echo "exit $?"
  rm -f ./brevi-python)
if [ "$1" = "--update" ]; then printf '%s\n' "$strict_out" > tests/strict.expected
elif [ "$strict_out" != "$(cat tests/strict.expected)" ]; then
  echo "FAIL strict"; printf '%s\n' "$strict_out" | diff tests/strict.expected - ; fail=1
else
  echo "ok   strict"
fi
#== lib split
# Standard-library programs must print what python3 prints, natively: whichever path the build takes
# (typed or dynamic), never CPython (pybridge or the python3 fallback).
for f in tests/lib/*.py; do
  exp="${f%.py}.expected"
  out=$(BREVI_PATH=1 timeout 600 $BREVI run "$f" --native 2>/tmp/brevi_lib_err)
  if [ "$1" = "--update" ]; then python3 "$f" > "$exp" 2>&1; continue; fi
  if grep -q '"path":"\(pybridge\|cpython\)"' /tmp/brevi_lib_err; then
    echo "FAIL $f (ran in CPython)"; cat /tmp/brevi_lib_err; fail=1
  elif [ "$out" != "$(cat "$exp")" ]; then
    echo "FAIL $f"; cat /tmp/brevi_lib_err; printf '%s\n' "$out" | diff "$exp" - ; fail=1
  else
    echo "ok   $f"
  fi
done
#== logging split
# Brevi's logging writes to stderr what CPython's writes, natively, but for traceback frames (Brevi writes an
# exception as it writes an uncaught one: the header and the last line).
for f in tests/logging/*.py; do
  exp="${f%.py}.expected"
  if [ "$1" = "--update" ]; then python3 "$f" 2>&1 | grep -v '^  \|^Call stack:' > "$exp"; continue; fi
  out=$(BREVI_PATH=1 timeout 600 $BREVI run "$f" --native 2>&1 | grep -v '^{"path"')
  if [ "$out" != "$(cat "$exp")" ]; then
    echo "FAIL $f"; printf '%s\n' "$out" | diff "$exp" - ; fail=1
  else
    echo "ok   $f"
  fi
done
#== bv_libs split
# Brevi files (.bv) that import libraries: Brevi's own copies run them, on the dynamic build's engine (engine.rs),
# never CPython. The .expected files match what the same programs print with CPython's libraries
# (BREVI_ENGINE=0, where python3 has them).
# A .bv file written in plain Python (tests/bv_py) gets the .py path: the dynamic build when the typed one
# refuses it, never python3.
for f in tests/bv_libs/*.bv tests/bv_py/*.bv; do
  exp="${f%.bv}.expected"
  out=$(cd "$(dirname "$f")" && BREVI_PATH=1 timeout 600 ../../$BREVI run "$(basename "$f")" 2>/tmp/brevi_bv_err)
  if [ "$1" = "--update" ]; then printf '%s\n' "$out" > "$exp"; continue; fi
  if grep -q '"path":"\(pybridge\|cpython\)"' /tmp/brevi_bv_err; then
    echo "FAIL $f (ran in CPython)"; cat /tmp/brevi_bv_err; fail=1
  elif [ "$out" != "$(cat "$exp")" ]; then
    echo "FAIL $f"; cat /tmp/brevi_bv_err; printf '%s\n' "$out" | diff "$exp" - ; fail=1
  else
    echo "ok   $f"
  fi
done
#== bv_libs_sandbox split
# The same Brevi files under --sandbox with no grant: Brevi's own libraries hold to the sandbox, so they run
# natively there too (never CPython, which a sandbox without a grant refuses).
for f in tests/bv_libs/*.bv; do
  out=$(cd "$(dirname "$f")" && BREVI_PATH=1 timeout 600 ../../$BREVI run --sandbox "$(basename "$f")" 2>/tmp/brevi_bv_err)
  if grep -q '"path":"\(pybridge\|cpython\)"' /tmp/brevi_bv_err || [ "$out" != "$(cat "${f%.bv}.expected")" ]; then
    echo "FAIL $f (--sandbox)"; cat /tmp/brevi_bv_err; fail=1
  else
    echo "ok   $f (--sandbox)"
  fi
done
#== numpy split
# numpy programs must print what CPython's numpy prints, byte for byte, with Brevi's own numpy: never CPython.
# The .expected files come from real numpy (numpy 2.5.4, see tools/upstream_pins.json; CI has none): `--update` needs it installed.
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
  # NAME_avx512.py: Brevi reproduces this numpy code for AVX-512 CPUs only; elsewhere the program goes to CPython
  case "$f" in *_avx512.py) [ "$np_avx512" = 0 ] && { echo "skip $f (needs AVX-512)"; continue; } ;; esac
  out=$(BREVI_PATH=1 timeout 600 $BREVI run "$f" --native 2>/tmp/brevi_np_err)
  if ! grep -q '"path":"dynamic"' /tmp/brevi_np_err; then
    echo "FAIL $f (not built with Brevi's numpy)"; cat /tmp/brevi_np_err; fail=1
  elif [ "$out" != "$(cat "$exp")" ]; then
    echo "FAIL $f"; printf '%s\n' "$out" | diff "$exp" - ; grep -v '"path"' /tmp/brevi_np_err | tail -5; fail=1
  else
    echo "ok   $f"
  fi
done
#== tiktoken split
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
#== tokenizers split
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
if [ "$1" != "--update" ]; then
  out=$($BREVI run --sandbox tests/tokenizers/sandbox/from_file.py 2>&1)
  case "$out" in
    *"tokenizers Tokenizer.from_file(file) needs allow read"*) echo "ok   sandbox tokenizers (from_file needs allow read)" ;;
    *) echo "FAIL sandbox tokenizers (from_file needs allow read)"; printf '%s\n' "$out"; fail=1 ;;
  esac
  out=$($BREVI run --sandbox --allow-read tests/tokenizers tests/tokenizers/sandbox/from_file.py 2>&1)
  case "$out" in
    *"is outside the folders granted to read"*) echo "ok   sandbox tokenizers (from_file held to the granted folders)" ;;
    *) echo "FAIL sandbox tokenizers (from_file held to the granted folders)"; printf '%s\n' "$out"; fail=1 ;;
  esac
fi
#== pydantic split
# pydantic programs must print what CPython's pydantic prints, with Brevi's own pydantic: never CPython. The
# .expected files come from pydantic 2.14.3 / pydantic-core 2.50.4 (CI has none): `--update` needs it installed.
for f in tests/pydantic/*.py; do
  exp="${f%.py}.expected"
  if [ "$1" = "--update" ]; then
    python3 -c "import pydantic" 2>/dev/null && python3 "$f" > "$exp" 2>&1
    continue
  fi
  out=$(BREVI_PATH=1 timeout 300 $BREVI run "$f" --native 2>/tmp/brevi_pyd_err)
  if ! grep -q '"path":"dynamic"' /tmp/brevi_pyd_err || grep -q '"py"' /tmp/brevi_pyd_err; then
    echo "FAIL $f (not built with Brevi's pydantic)"; cat /tmp/brevi_pyd_err; fail=1
  elif [ "$out" != "$(cat "$exp")" ]; then
    echo "FAIL $f"; printf '%s\n' "$out" | diff "$exp" - ; grep -v '"path"' /tmp/brevi_pyd_err | tail -5; fail=1
  else
    echo "ok   $f"
  fi
done
if [ "$1" != "--update" ]; then
  out=$($BREVI run --sandbox tests/pydantic/sandbox/schema.py 2>&1)
  case "$out" in
    *"pydantic parse_obj needs allow native"*) echo "ok   sandbox pydantic (what the copy lacks is refused)" ;;
    *) echo "FAIL sandbox pydantic (what the copy lacks is refused)"; printf '%s\n' "$out"; fail=1 ;;
  esac
fi
#== jinja2 split
# jinja2 programs must print what CPython's jinja2 prints, with Brevi's own jinja2 and markupsafe: never CPython.
# The .expected files come from jinja2 3.1.6 / markupsafe 3.0.3 (CI has none): `--update` needs them installed.
for f in tests/jinja2/*.py; do
  exp="${f%.py}.expected"
  if [ "$1" = "--update" ]; then
    python3 -c "import jinja2" 2>/dev/null && python3 "$f" > "$exp" 2>&1
    continue
  fi
  out=$(BREVI_PATH=1 timeout 300 $BREVI run "$f" --native 2>/tmp/brevi_j2_err)
  if ! grep -q '"path":"dynamic"' /tmp/brevi_j2_err || grep -q '"py"' /tmp/brevi_j2_err; then
    echo "FAIL $f (not built with Brevi's jinja2)"; cat /tmp/brevi_j2_err; fail=1
  elif [ "$out" != "$(cat "$exp")" ]; then
    echo "FAIL $f"; printf '%s\n' "$out" | diff "$exp" - ; grep -v '"path"' /tmp/brevi_j2_err | tail -5; fail=1
  else
    echo "ok   $f"
  fi
done
if [ "$1" != "--update" ]; then
  out=$($BREVI run --sandbox tests/jinja2/sandbox/files.py 2>&1)
  case "$out" in
    *"jinja2 FileSystemLoader needs allow native"*) echo "ok   sandbox jinja2 (what the copy lacks is refused)" ;;
    *) echo "FAIL sandbox jinja2 (what the copy lacks is refused)"; printf '%s\n' "$out"; fail=1 ;;
  esac
fi
#== polars split
# polars programs must print what CPython's polars prints (stdout), with Brevi's own polars: never CPython.
# The .expected files come from polars 2.1.0 (CI has none): `--update` needs it installed.
for f in tests/polars/*.py; do
  exp="${f%.py}.expected"
  if [ "$1" = "--update" ]; then
    python3 -c "import polars" 2>/dev/null && python3 "$f" > "$exp" 2>/dev/null
    continue
  fi
  out=$(BREVI_PATH=1 timeout 600 $BREVI run "$f" --native 2>/tmp/brevi_pl_err)
  if ! grep -q '"path":"dynamic"' /tmp/brevi_pl_err || grep -q '"py"' /tmp/brevi_pl_err; then
    echo "FAIL $f (not built with Brevi's polars)"; cat /tmp/brevi_pl_err; fail=1
  elif [ "$out" != "$(cat "$exp")" ]; then
    echo "FAIL $f"; printf '%s\n' "$out" | diff "$exp" - ; grep -v '"path"' /tmp/brevi_pl_err | tail -5; fail=1
  else
    echo "ok   $f"
  fi
done
if [ "$1" != "--update" ]; then
  out=$($BREVI run --sandbox tests/polars/sandbox/parquet.py 2>&1)
  case "$out" in
    *"polars parquet needs allow native"*) echo "ok   sandbox polars (what the copy lacks is refused)" ;;
    *) echo "FAIL sandbox polars (what the copy lacks is refused)"; printf '%s\n' "$out"; fail=1 ;;
  esac
  out=$($BREVI run --sandbox tests/polars/sandbox/files.py 2>&1)
  case "$out" in
    *"polars write_csv(file) needs allow write"*"polars read_csv(file) needs allow read"*) echo "ok   sandbox polars (files need allow read, write)" ;;
    *) echo "FAIL sandbox polars (files need allow read, write)"; printf '%s\n' "$out"; fail=1 ;;
  esac  # --allow-read / --allow-write DIR: only inside the granted folders
  out=$($BREVI run --sandbox --allow-read tests/sandbox/scoped/in,tests/sandbox/scoped/out --allow-write tests/sandbox/scoped/out tests/polars/sandbox/scoped.py 2>/dev/null)
  if [ "$out" = "$(cat tests/polars/sandbox/scoped.expected)" ]; then echo "ok   sandbox polars (held to the granted folders)"; else echo "FAIL sandbox polars (held to the granted folders)"; printf '%s\n' "$out"; fail=1; fi
fi
#== sqlite3 split
# sqlite3 programs must print what CPython's sqlite3 prints (stdout), with Brevi's own sqlite3 over the system's
# SQLite: never CPython. `--update` writes the .expected files with python3.
for f in tests/sqlite3/*.py; do
  exp="${f%.py}.expected"
  if [ "$1" = "--update" ]; then
    python3 "$f" > "$exp" 2>/dev/null
    continue
  fi
  out=$(BREVI_PATH=1 timeout 600 $BREVI run "$f" --native 2>/tmp/brevi_sq_err)
  if ! grep -q '"path":"dynamic"' /tmp/brevi_sq_err || grep -q '"py"' /tmp/brevi_sq_err; then
    echo "FAIL $f (not built with Brevi's sqlite3)"; cat /tmp/brevi_sq_err; fail=1
  elif [ "$out" != "$(cat "$exp")" ]; then
    echo "FAIL $f"; printf '%s\n' "$out" | diff "$exp" - ; grep -v '"path"' /tmp/brevi_sq_err | tail -5; fail=1
  else
    echo "ok   $f"
  fi
done
if [ "$1" != "--update" ]; then
  out=$($BREVI run --sandbox tests/sqlite3/sandbox/files.py 2>&1)
  case "$out" in
    *"sqlite3 connect(file) needs allow read"*"sqlite3 connect(file) needs allow write"*) echo "ok   sandbox sqlite3 (files need allow read, write)" ;;
    *) echo "FAIL sandbox sqlite3 (files need allow read, write)"; printf '%s\n' "$out"; fail=1 ;;
  esac
  # SQL that opens files itself (ATTACH, VACUUM INTO, a vfs= URI, load_extension) is refused by SQLite's file layer
  out=$($BREVI run --sandbox tests/sqlite3/sandbox/attach.py 2>/dev/null)
  if [ "$out" = "$(cat tests/sqlite3/sandbox/attach.expected)" ] && ! ls /tmp/brevi_sb_attach.db /tmp/brevi_sb_vacuum.db >/dev/null 2>&1; then
    echo "ok   sandbox sqlite3 (SQL reaches no file)"
  else
    echo "FAIL sandbox sqlite3 (SQL reaches no file)"; printf '%s\n' "$out"; fail=1
  fi
  out=$($BREVI run --sandbox --allow-read tests/sandbox/scoped/in,tests/sandbox/scoped/out --allow-write tests/sandbox/scoped/out tests/sqlite3/sandbox/scoped.py 2>/dev/null)
  if [ "$out" = "$(cat tests/sqlite3/sandbox/scoped.expected)" ] && ! ls tests/sandbox/scoped/x.db tests/sandbox/scoped/in/x.db >/dev/null 2>&1; then
    echo "ok   sandbox sqlite3 (held to the granted folders)"
  else
    echo "FAIL sandbox sqlite3 (held to the granted folders)"; printf '%s\n' "$out"; fail=1
  fi
fi
#== duckdb
# duckdb programs must print what CPython's duckdb 2.0.0 prints (stdout), with Brevi's own duckdb over DuckDB's
# engine (libduckdb, or the duckdb package's library): never CPython. Skipped without the duckdb package (PR CI has
# none; releases and the daily run install it and fail without it). `--update` writes the .expected files with python3.
if has duckdb; then
  for f in tests/duckdb/*.py; do
    exp="${f%.py}.expected"
    if [ "$1" = "--update" ]; then
      python3 "$f" > "$exp" 2>/dev/null
      continue
    fi
    out=$(BREVI_PATH=1 timeout 600 $BREVI run "$f" --native 2>/tmp/brevi_dk_err)
    if ! grep -q '"path":"dynamic"' /tmp/brevi_dk_err || grep -q '"py"' /tmp/brevi_dk_err; then
      echo "FAIL $f (not built with Brevi's duckdb)"; cat /tmp/brevi_dk_err; fail=1
    elif [ "$out" != "$(cat "$exp")" ]; then
      echo "FAIL $f"; printf '%s\n' "$out" | diff "$exp" - ; grep -v '"path"' /tmp/brevi_dk_err | tail -5; fail=1
    else
      echo "ok   $f"
    fi
  done
  if [ "$1" != "--update" ]; then
    out=$($BREVI run --sandbox tests/duckdb/sandbox/files.py 2>&1)
    case "$out" in
      *"line\":6,\"msg\":\"duckdb connect(file) needs allow read"*"line\":7,\"msg\":\"duckdb connect(file) needs allow write"*) echo "ok   sandbox duckdb (files need allow read, write)" ;;
      *) echo "FAIL sandbox duckdb (files need allow read, write)"; printf '%s\n' "$out"; fail=1 ;;
    esac
    # SQL that reads or writes files (COPY, read_csv, ATTACH, glob), installs extensions or unlocks the settings is
    # refused by DuckDB under --sandbox
    out=$($BREVI run --sandbox tests/duckdb/sandbox/sql.py 2>/dev/null)
    if [ "$out" = "$(cat tests/duckdb/sandbox/sql.expected)" ] && ! ls /tmp/brevi_sb_duck.csv /tmp/brevi_sb_duck_attach.db >/dev/null 2>&1; then
      echo "ok   sandbox duckdb (SQL reaches no file)"
    else
      echo "FAIL sandbox duckdb (SQL reaches no file)"; printf '%s\n' "$out"; fail=1
    fi
  fi
else
  echo "skip tests/duckdb (no duckdb package)"
fi
#== safetensors split
# safetensors programs must print what CPython's safetensors 0.8 (numpy framework) prints, with Brevi's own
# safetensors over its native numpy: never CPython. Brevi needs no package; `--update` needs safetensors and numpy.
for f in tests/safetensors/*.py; do
  exp="${f%.py}.expected"
  if [ "$1" = "--update" ]; then
    python3 -c "import safetensors, numpy" 2>/dev/null && python3 "$f" > "$exp" 2>/dev/null
    continue
  fi
  out=$(BREVI_PATH=1 timeout 600 $BREVI run "$f" --native 2>/tmp/brevi_st_err)
  if ! grep -q '"path":"dynamic"' /tmp/brevi_st_err || grep -q '"py"' /tmp/brevi_st_err; then
    echo "FAIL $f (not built with Brevi's safetensors)"; cat /tmp/brevi_st_err; fail=1
  elif [ "$out" != "$(cat "$exp")" ]; then
    echo "FAIL $f"; printf '%s\n' "$out" | diff "$exp" - ; grep -v '"path"' /tmp/brevi_st_err | tail -5; fail=1
  else
    echo "ok   $f"
  fi
done
if [ "$1" != "--update" ]; then
  out=$($BREVI run --sandbox tests/safetensors/sandbox/files.py 2>&1)
  case "$out" in
    *"save_file(file) needs allow write"*"load_file(file) needs allow read"*"safe_open(file) needs allow read"*) echo "ok   sandbox safetensors (files need allow read, write)" ;;
    *) echo "FAIL sandbox safetensors (files need allow read, write)"; printf '%s\n' "$out"; fail=1 ;;
  esac
fi
#== onnxruntime
# onnxruntime programs must print what CPython's onnxruntime 1.31 prints (1.29 prints the same), with Brevi's own onnxruntime over ONNX
# Runtime's C API (the onnxruntime package's library, or libonnxruntime): never CPython. Skipped without the
# onnxruntime package (PR CI has none; releases and the daily run install it and fail without it). tests/onnxruntime/models/make_models.py (needs onnx) wrote the models.
if has onnxruntime; then
  for f in tests/onnxruntime/*.py; do
    exp="${f%.py}.expected"
    if [ "$1" = "--update" ]; then
      python3 "$f" > "$exp" 2>/dev/null
      continue
    fi
    out=$(BREVI_PATH=1 timeout 600 $BREVI run "$f" --native 2>/tmp/brevi_ort_err)
    if ! grep -q '"path":"dynamic"' /tmp/brevi_ort_err || grep -q '"py"' /tmp/brevi_ort_err; then
      echo "FAIL $f (not built with Brevi's onnxruntime)"; cat /tmp/brevi_ort_err; fail=1
    elif [ "$out" != "$(cat "$exp")" ]; then
      echo "FAIL $f"; printf '%s\n' "$out" | diff "$exp" - ; grep -v '"path"' /tmp/brevi_ort_err | tail -5; fail=1
    else
      echo "ok   $f"
    fi
  done
  if [ "$1" != "--update" ]; then
    out=$($BREVI run --sandbox tests/onnxruntime/sandbox/files.py 2>&1)
    case "$out" in
      *"line\":5,\"msg\":\"onnxruntime InferenceSession(model file) needs allow read"*"line\":7,\"msg\":\"onnxruntime files it writes"*"line\":8,"*) echo "ok   sandbox onnxruntime (files need allow read, write)" ;;
      *) echo "FAIL sandbox onnxruntime (files need allow read, write)"; printf '%s\n' "$out"; fail=1 ;;
    esac
    # a model given as bytes needs no grant, unless its weights are in external data files
    out=$($BREVI run --sandbox tests/onnxruntime/sandbox/bytes.py 2>/dev/null)
    if [ "$out" = "$(cat tests/onnxruntime/sandbox/bytes.expected)" ]; then
      echo "ok   sandbox onnxruntime (external data needs allow read)"
    else
      echo "FAIL sandbox onnxruntime (external data needs allow read)"; printf '%s\n' "$out"; fail=1
    fi
  fi
else
  echo "skip tests/onnxruntime (no onnxruntime package)"
fi
#== http split
# requests and httpx programs must print what CPython's requests 2.34 and httpx 0.28 print, with Brevi's own copies
# over native sockets and TLS: never CPython. They talk to tests/http/server.py (HTTP on 18080, HTTPS on 18081)
# and pass verify= themselves, so a CA bundle in the environment changes nothing. `--update` needs both libraries.
# tests/check_parallel.py starts this server once for all its jobs (CHECK_SERVERS=1).
if [ -z "$CHECK_SERVERS" ]; then
  python3 tests/http/server.py 18080 > /tmp/brevi_http_srv 2>&1 &
  srv=$!
  for i in 1 2 3 4 5 6 7 8 9 10; do grep -q ready /tmp/brevi_http_srv 2>/dev/null && break; sleep 0.5; done
fi
for f in tests/http/requests_*.py tests/http/httpx_*.py; do
  exp="${f%.py}.expected"
  if [ "$1" = "--update" ]; then
    python3 -c "import requests, httpx" 2>/dev/null && python3 "$f" > "$exp" 2>/dev/null
    continue
  fi
  out=$(BREVI_PATH=1 timeout 300 $BREVI run "$f" --native 2>/tmp/brevi_http_err)
  if ! grep -q '"path":"dynamic"' /tmp/brevi_http_err || grep -q '"py"' /tmp/brevi_http_err; then
    echo "FAIL $f (not built with Brevi's requests / httpx)"; cat /tmp/brevi_http_err; fail=1
  elif [ "$out" != "$(cat "$exp")" ]; then
    echo "FAIL $f"; printf '%s\n' "$out" | diff "$exp" - ; grep -v '"path"' /tmp/brevi_http_err | tail -5; fail=1
  else
    echo "ok   $f"
  fi
done
# Under --sandbox the network needs the caller's grant: none refuses the build, --allow-net lets in only the
# hosts it names (127.0.0.1 here, so localhost is refused when the program connects).
if [ "$1" != "--update" ]; then
  out=$($BREVI run --sandbox tests/http/sandbox_net.py 2>&1; echo "exit $?")
  if printf '%s\n' "$out" | grep -q 'needs allow net' && printf '%s\n' "$out" | grep -qx 'exit 1'; then echo "ok   sandbox net (no grant)"; else echo "FAIL sandbox net (no grant)"; printf '%s\n' "$out"; fail=1; fi
  out=$(timeout 300 $BREVI run --sandbox --allow-net 127.0.0.1 tests/http/sandbox_net.py 2>/dev/null)
  if [ "$out" = "$(cat tests/http/sandbox_net.expected)" ]; then echo "ok   sandbox net (--allow-net 127.0.0.1)"; else echo "FAIL sandbox net (--allow-net 127.0.0.1)"; printf '%s\n' "$out" | diff tests/http/sandbox_net.expected -; fail=1; fi
  out=$(timeout 300 $BREVI run --sandbox --allow-net 127.0.0.1 --limit net=20kb tests/http/sandbox_net_limit.py 2>&1; echo "exit $?")
  case "$out" in
    *'"on":"net"'*"exit 124") echo "ok   sandbox net (--limit net)" ;;
    *) echo "FAIL sandbox net (--limit net)"; printf '%s\n' "$out" | tail -3; fail=1 ;;
  esac
fi
kill $srv 2>/dev/null
#== llm split
# anthropic and openai programs must print what CPython's anthropic 2.6 and openai 3.49 print (models, streams,
# tool calls, errors, retries), with Brevi's own copies over its httpx: never CPython. They talk to the mock API in
# tests/llm/server.py on 18090. `--update` needs both SDKs (and tools/gen_llm_types.py rewrites the types).
# tests/check_parallel.py starts this server once for all its jobs (CHECK_SERVERS=1).
if [ -z "$CHECK_SERVERS" ]; then
  python3 tests/llm/server.py 18090 > /tmp/brevi_llm_srv 2>&1 &
  srv=$!
  for i in 1 2 3 4 5 6 7 8 9 10; do grep -q listening /tmp/brevi_llm_srv 2>/dev/null && break; sleep 0.5; done
fi
for f in tests/llm/anthropic_*.py tests/llm/openai_*.py; do
  exp="${f%.py}.expected"
  if [ "$1" = "--update" ]; then
    python3 -c "import anthropic, openai" 2>/dev/null && env -u ANTHROPIC_API_KEY -u OPENAI_API_KEY python3 "$f" > "$exp" 2>&1
    continue
  fi
  out=$(env -u ANTHROPIC_API_KEY -u OPENAI_API_KEY BREVI_PATH=1 timeout 300 $BREVI run "$f" --native 2>/tmp/brevi_llm_err)
  if ! grep -q '"path":"dynamic"' /tmp/brevi_llm_err || grep -q '"py"' /tmp/brevi_llm_err; then
    echo "FAIL $f (not built with Brevi's anthropic / openai)"; cat /tmp/brevi_llm_err; fail=1
  elif [ "$out" != "$(cat "$exp")" ]; then
    echo "FAIL $f"; printf '%s\n' "$out" | diff "$exp" - ; grep -v '"path"' /tmp/brevi_llm_err | tail -5; fail=1
  else
    echo "ok   $f"
  fi
done
# Under --sandbox they need the network granted, as requests and httpx do.
if [ "$1" != "--update" ]; then
  out=$($BREVI run --sandbox tests/llm/openai_async.py 2>&1; echo "exit $?")
  if printf '%s\n' "$out" | grep -q 'import openai (the network) needs allow net'; then echo "ok   sandbox llm (no grant)"; else echo "FAIL sandbox llm (no grant)"; printf '%s\n' "$out"; fail=1; fi
  out=$(env -u OPENAI_API_KEY timeout 300 $BREVI run --sandbox --allow-net 127.0.0.1 tests/llm/openai_async.py 2>/dev/null)
  if [ "$out" = "$(cat tests/llm/openai_async.expected)" ]; then echo "ok   sandbox llm (--allow-net 127.0.0.1)"; else echo "FAIL sandbox llm (--allow-net 127.0.0.1)"; printf '%s\n' "$out" | diff tests/llm/openai_async.expected -; fail=1; fi
fi
kill $srv 2>/dev/null
#== mcp_sdk split
# MCP servers on the official mcp SDK (MCPServer, the low-level Server, stdio) must answer what CPython's mcp 2.3.0
# answers, built with Brevi's own copy: never CPython. tests/mcp_sdk/drive.py plays each NAME.CASE.jsonl to the
# server in lockstep; the .expected transcripts are CPython's, stderr left out. `--update` needs mcp installed.
for f in tests/mcp_sdk/*.py; do
  case "$f" in */drive.py|*/devrun.py) continue ;; esac
  n=$(basename "$f" .py)
  if [ "$1" = "--update" ]; then
    python3 -c "import mcp" 2>/dev/null || continue
    for j in tests/mcp_sdk/$n.*.jsonl; do python3 tests/mcp_sdk/drive.py "$j" -- python3 "$f" | grep -v '^stderr: ' > "${j%.jsonl}.expected"; done
    continue
  fi
  if ! BREVI_PATH=1 $BREVI build "$f" -o /tmp/brevi_mcp_$n 2>/tmp/brevi_mcp_err || ! grep -q '"path":"dynamic"' /tmp/brevi_mcp_err || grep -q '"py"' /tmp/brevi_mcp_err; then
    echo "FAIL $f (not built with Brevi's mcp)"; cat /tmp/brevi_mcp_err; fail=1; continue
  fi
  for j in tests/mcp_sdk/$n.*.jsonl; do
    out=$(python3 tests/mcp_sdk/drive.py "$j" -- /tmp/brevi_mcp_$n | grep -v '^stderr: ')
    if [ "$out" != "$(cat "${j%.jsonl}.expected")" ]; then
      echo "FAIL $j"; printf '%s\n' "$out" | diff "${j%.jsonl}.expected" - | head -20; fail=1
    else
      echo "ok   $j"
    fi
  done
  rm -f /tmp/brevi_mcp_$n
done
#== mcp_http
# The same over HTTP (MCPServer.run(transport="streamable-http" / "sse")): tests/mcp_sdk/http/drive.py sends raw
# requests and prints status, headers and bodies; the .expected transcripts are CPython's (mcp 2.3.0 on uvicorn).
# Then the official client (streamable HTTP and SSE) against the build, where python3 has mcp.
H=tests/mcp_sdk/http
if [ "$1" = "--update" ]; then
  if python3 -c "import mcp, uvicorn" 2>/dev/null; then
    for m in "streamable streamable-http" "json streamable-http json_response=1" "stateless streamable-http stateless_http=1" "sse sse"; do
      python3 $H/drive.py ${m%% *} 18871 -- python3 $H/server.py ${m#* } 18871 > $H/${m%% *}.expected
    done
  fi
elif ! BREVI_PATH=1 $BREVI build $H/server.py -o /tmp/brevi_mcp_http 2>/tmp/brevi_mcp_err || ! grep -q '"path":"dynamic"' /tmp/brevi_mcp_err || grep -q '"py"' /tmp/brevi_mcp_err; then
  echo "FAIL $H/server.py (not built with Brevi's mcp)"; cat /tmp/brevi_mcp_err; fail=1
else
  for m in "streamable streamable-http" "json streamable-http json_response=1" "stateless streamable-http stateless_http=1" "sse sse"; do
    c=${m%% *}
    out=$(timeout 300 python3 $H/drive.py $c 18872 -- /tmp/brevi_mcp_http ${m#* } 18872)
    if [ "$out" != "$(cat $H/$c.expected)" ]; then echo "FAIL mcp http ($c)"; printf '%s\n' "$out" | diff $H/$c.expected - | head -20; fail=1; else echo "ok   mcp http ($c)"; fi
  done
  if has "mcp.client.streamable_http, mcp.client.sse"; then
    for k in streamable sse; do
      t=streamable-http; [ $k = sse ] && t=sse
      out=$(timeout 300 python3 $H/client.py $k 18873 -- /tmp/brevi_mcp_http $t 18873 2>&1 | tail -1)
      case "$out" in "['http-demo', ['add', 'slow', 'touch'], {'result': 42}, 'done 3', [(1.0, 3.0), (2.0, 3.0), (3.0, 3.0)]]") echo "ok   mcp http official client ($k)" ;;
        *) echo "FAIL mcp http official client ($k)"; printf '%s\n' "$out"; fail=1 ;; esac
    done
  fi
  rm -f /tmp/brevi_mcp_http
fi
# brevi verify plays a server's .jsonl session in lockstep and certifies it (only where CPython has mcp installed).
if [ "$1" != "--update" ] && has mcp; then
  for c in basics.lead lowlevel.calls rich.calls; do
    o=$($BREVI verify tests/mcp_sdk/${c%%.*}.py --input tests/mcp_sdk/$c.jsonl 2>&1 | tail -1)
    case "$o" in '{"verify":"identical"'*) echo "ok   brevi verify tests/mcp_sdk/$c.jsonl" ;; *) echo "FAIL brevi verify tests/mcp_sdk/$c.jsonl"; printf '%s\n' "$o"; fail=1 ;; esac
  done
fi
# Under --sandbox a stdio server needs no grant (it only reads stdin and writes stdout); what the copy lacks is refused.
if [ "$1" != "--update" ]; then
  out=$(head -3 tests/mcp_sdk/basics.calls.jsonl | timeout 300 $BREVI run --sandbox tests/mcp_sdk/basics.py 2>/dev/null | tail -1)
  case "$out" in '{"jsonrpc":"2.0","id":2,"result":{"tools":[{"description":"Add two numbers."'*) echo "ok   sandbox mcp (stdio, no grant)" ;;
    *) echo "FAIL sandbox mcp (stdio, no grant)"; printf '%s\n' "$out"; fail=1 ;; esac
  out=$($BREVI run --sandbox tests/mcp_sdk/sandbox/http.py 2>&1)
  case "$out" in *"an MCP server on an HTTP port"*"needs allow net"*) echo "ok   sandbox mcp (an HTTP server needs the net grant)" ;;
    *) echo "FAIL sandbox mcp (an HTTP server needs the net grant)"; printf '%s\n' "$out"; fail=1 ;; esac
  out=$(timeout 300 $BREVI run --sandbox --allow-net example.com tests/mcp_sdk/sandbox/http.py 2>&1)
  case "$out" in *"cannot listen on it"*) echo "ok   sandbox mcp (listens only on a granted host)" ;;
    *) echo "FAIL sandbox mcp (listens only on a granted host)"; printf '%s\n' "$out"; fail=1 ;; esac
  out=$($BREVI run --sandbox tests/mcp_sdk/sandbox/lacks.py 2>&1)
  case "$out" in *"needs allow native"*) echo "ok   sandbox mcp (what the copy lacks is refused)" ;;
    *) echo "FAIL sandbox mcp (what the copy lacks is refused)"; printf '%s\n' "$out"; fail=1 ;; esac
fi
#== dyn split
# Python programs built over dynamic values (BREVI_DYN=1 skips the typed build) must print what python3 prints.
for f in tests/dyn/*.py; do
  exp="${f%.py}.expected"
  inp="${f%.py}.in"; [ -f "$inp" ] || inp=/dev/null
  out=$(BREVI_DYN=1 $BREVI run "$f" < "$inp" 2>&1)
  if [ "$1" = "--update" ]; then python3 "$f" < "$inp" > "$exp" 2>&1; continue; fi
  if [ "$out" != "$(cat "$exp")" ]; then
    echo "FAIL $f"; printf '%s\n' "$out" | diff "$exp" - ; fail=1
  else
    echo "ok   $f"
  fi
done
# sys.settrace is compiled only for the statements whose events are known to be CPython 3.13's (dynpy/trace_check.rs,
# tools/trace_fuzz.py); anything else in a program that uses it is refused when built, never traced differently. dis
# is refused: a compiled function has no bytecode.
t=$(mktemp -d)
printf 'import sys\ndef f():\n    for i in range(2):\n        pass\nsys.settrace(None)\n' > "$t/loop.py"
printf 'import sys\ndef t(fr, ev, arg):\n    print(fr.f_locals)\n    return t\nsys.settrace(t)\n' > "$t/flocals.py"
printf 'import sys\nsys.settrace(lambda *a: None)\n' > "$t/lam.py"
printf 'import sys\ndef f():\n    try:\n        x = 1\n    except ValueError:\n        pass\nsys.settrace(None)\n' > "$t/exc.py"
printf 'import dis\ndef f():\n    return 1\nprint(list(dis.get_instructions(f)))\n' > "$t/dis.py"
for c in "loop:this statement inside a def or class is not supported in a program that uses sys.settrace" \
         "flocals:the f_locals attribute is not supported in a program that uses sys.settrace" \
         "lam:a lambda is not supported in a program that uses sys.settrace" \
         "exc:this statement inside a def or class is not supported in a program that uses sys.settrace" \
         "dis:module 'dis' is not supported in dynamic builds"; do
  n=${c%%:*}; want=${c#*:}
  o=$(BREVI_DYN=1 $BREVI build "$t/$n.py" -o "$t/$n.bin" 2>&1)
  case "$o" in *"$want"*) echo "ok   dynamic build refuses $n with sys.settrace/dis" ;; *) echo "FAIL dynamic build refuses $n with sys.settrace/dis"; printf '%s\n' "$o" | tail -3; fail=1 ;; esac
done
rm -rf "$t"
# A str with a lone surrogate on a path not yet written for one stops the program, even inside try/except: never a
# different answer (handoffs/string-model-design.md).
t=$(mktemp -d)
# A module attribute or repr Brevi cannot give as CPython does is refused, never a different answer: the import
# machinery's __spec__ when the program is built, or read through a variable when the program runs; the repr of
# sys.stdout or a decimal context likewise. A library module's repr is the build machine's CPython's.
printf 'X = 1\n' > "$t/helper.py"
printf 'import helper\nprint(helper.__spec__)\n' > "$t/m1.py"
o=$(BREVI_DYN=1 $BREVI run "$t/m1.py" 2>&1)
case "$o" in *"helper.__spec__ is not supported in dynamic builds"*) echo "ok   a program module's __spec__ is refused when built" ;; *) echo "FAIL a program module's __spec__"; printf '%s\n' "$o"; fail=1 ;; esac
printf 'import helper\nm = helper\nprint(m.__loader__)\n' > "$t/m2.py"
o=$(BREVI_DYN=1 $BREVI run "$t/m2.py" 2>&1)
case "$o" in *"NotImplementedError: brevi: helper.__loader__ is not supported in dynamic builds") echo "ok   a program module's __loader__ through a variable stops" ;; *) echo "FAIL a program module's __loader__ through a variable"; printf '%s\n' "$o"; fail=1 ;; esac
printf 'import math, sys, json, re\nprint(sys, f"{math}", [json])\nd = {"m": re}\nprint(d, str(re) == repr(re))\n' > "$t/m3.py"
o=$(BREVI_DYN=1 $BREVI build "$t/m3.py" -o "$t/m3" 2>&1 && "$t/m3" 2>&1); w=$(python3 "$t/m3.py" 2>&1)
if [ "$o" = "$w" ]; then echo "ok   a library module's repr is CPython's"; else echo "FAIL a library module's repr"; printf '%s\n' "$o" "$w"; fail=1; fi
printf 'import sys\nprint(f"{sys.stdout}")\n' > "$t/m4.py"
o=$(BREVI_DYN=1 $BREVI build "$t/m4.py" -o "$t/m4" 2>&1)
case "$o" in *"the repr of sys.stdout is not supported in dynamic builds"*) echo "ok   the repr of sys.stdout is refused when built" ;; *) echo "FAIL the repr of sys.stdout when built"; printf '%s\n' "$o"; fail=1 ;; esac
printf 'import decimal\nc = [decimal.getcontext()]\nprint(c)\n' > "$t/m5.py"
o=$(BREVI_DYN=1 $BREVI run "$t/m5.py" 2>&1)
case "$o" in *"NotImplementedError: brevi: the repr of decimal.context is not supported in dynamic builds") echo "ok   a decimal context's repr the build cannot see stops" ;; *) echo "FAIL a decimal context's repr at run time"; printf '%s\n' "$o"; fail=1 ;; esac
printf 'import json\ntry:\n    print(json.loads(chr(34) + chr(0xd800) + chr(34)))\nexcept BaseException:\n    print("caught")\nprint("after")\n' > "$t/s.py"
o=$(BREVI_DYN=1 $BREVI run "$t/s.py" 2>&1); c=$?
case "$c:$o" in "1:NotImplementedError: brevi: this operation on a str with a lone surrogate (U+D800..U+DFFF) is not supported in dynamic builds yet"*) echo "ok   a lone surrogate on an unwritten path stops the program" ;; *) echo "FAIL a lone surrogate on an unwritten path ($c)"; printf '%s\n' "$o"; fail=1 ;; esac
# locals() in an Enum's body is enum.EnumDict, whose writes check names (a member set twice is a TypeError); Brevi's
# namespace is a plain dict, so a class that may be an Enum and calls locals() is refused when it builds.
printf 'import enum\nclass E(enum.Enum):\n    A = 1\n    locals().update({"B": 2}, **{"A": 3})\nprint(list(E))\n' > "$t/el.py"
o=$(BREVI_DYN=1 $BREVI build "$t/el.py" -o "$t/el" 2>&1)
case "$o" in *"locals() in the body of a class that may be an Enum is not supported in dynamic builds"*) echo "ok   locals() in an Enum body is refused" ;; *) echo "FAIL locals() in an Enum body"; printf '%s\n' "$o"; fail=1 ;; esac
printf 'def f():\n    "\\ud800"\n' > "$t/d.py"
o=$(BREVI_DYN=1 $BREVI run "$t/d.py" 2>&1)
case "$o" in *"a docstring with a lone surrogate (U+D800..U+DFFF) is not supported"*) echo "ok   a docstring with a lone surrogate is refused" ;; *) echo "FAIL a docstring with a lone surrogate"; printf '%s\n' "$o"; fail=1 ;; esac
# python3's stdout under the C locale is UTF-8 with surrogateescape: \udc80 is written as the byte 0x80, and \ud800
# still raises (the same answer both builds give: compare bytes with python3's).
printf 'import sys\nprint("a" + chr(0xdc80) + "b")\nsys.stdout.write(chr(0xdcff) + "\\n")\ntry:\n    print(chr(0xd800))\nexcept UnicodeEncodeError as e:\n    print(e)\n' > "$t/c.py"
want=$(env -u LANG -u LC_ALL -u LC_CTYPE -u PYTHONIOENCODING -u PYTHONUTF8 python3 "$t/c.py" 2>&1 | od -An -tx1)
got=$(env -u LANG -u LC_ALL -u LC_CTYPE -u PYTHONIOENCODING -u PYTHONUTF8 BREVI_DYN=1 $BREVI run "$t/c.py" 2>&1 | od -An -tx1)
if [ -n "$want" ] && [ "$got" = "$want" ]; then echo "ok   a lone surrogate on stdout under the C locale"; else echo "FAIL a lone surrogate on stdout under the C locale"; echo "want $want"; echo "got  $got"; fail=1; fi
# Under --sandbox the program reads no environment, so it cannot name stdout's error handler: it stops instead.
o=$(BREVI_DYN=1 $BREVI run --sandbox "$t/c.py" 2>&1); c=$?
case "$c:$o" in "1:NotImplementedError: brevi: this operation on a str with a lone surrogate"*) echo "ok   a lone surrogate on stdout under --sandbox stops" ;; *) echo "FAIL a lone surrogate on stdout under --sandbox ($c)"; printf '%s\n' "$o"; fail=1 ;; esac
# What a native coroutine cannot give is refused when the program is built (test_coroutines): its frame and code
# objects, and types.coroutine (which marks a code object) in every spelling the build can see, except as the
# innermost decorator of a generator function (tests/lib/coroutine_protocol.py).
printf 'async def f():\n    pass\nc = f()\nprint(c.cr_frame)\n' > "$t/cr.py"
printf 'import types as t\ndef g():\n    yield 1\nh = t.coroutine(g)\n' > "$t/tc.py"
printf 'from types import coroutine\n' > "$t/tf.py"
printf 'import types\n@types.coroutine\ndef g():\n    return 1\n' > "$t/tn.py"
for x in "cr:the cr_frame attribute of a coroutine" "tc:types.coroutine" "tf:types.coroutine" "tn:types.coroutine on a function that is not a generator function"; do
  o=$(BREVI_DYN=1 $BREVI build "$t/${x%%:*}.py" -o "$t/prog" 2>&1)
  case "$o" in *"${x#*:}"*"not supported in dynamic builds"*) echo "ok   refused: ${x#*:}" ;; *) echo "FAIL not refused: ${x#*:}"; printf '%s\n' "$o"; fail=1 ;; esac
done
# A class on Generic or Protocol (`class C[T]` included) has them left out of its bases here: reading its bases,
# MRO or __dict__ is refused, and so is a subscript of a PEP 695 class used as a value (test_type_params).
for x in "C.__bases__" "D.__mro__" "C.mro()" "getattr(C, '__orig_bases__')" "vars(C)" "dir(D())" "P.__bases__" "C[int]"; do
  printf 'from typing import Protocol\nclass P(Protocol): pass\nclass C[T]: pass\nclass D(C): pass\nprint(%s)\n' "$x" > "$t/g.py"
  o=$(BREVI_DYN=1 $BREVI build "$t/g.py" -o "$t/prog" 2>&1)
  case "$o" in *"needs CPython's typing"*) echo "ok   refused: $x of a generic class" ;; *) echo "FAIL not refused: $x of a generic class"; printf '%s\n' "$o"; fail=1 ;; esac
done
rm -rf "$t"
#== bridge split
# Python libraries the dynamic build lacks (tests/bridge/brlib stands in for a third-party one) run in CPython
# inside a dynamic build: output must match python3, the path line must name them, and nothing may run fully
# in CPython. Each runs by default and with BREVI_DYN=1.
for f in tests/bridge/*.py; do
  exp="${f%.py}.expected"
  if [ "$1" = "--update" ]; then (cd tests/bridge && python3 "$(basename "$f")") > "$exp" 2>&1; continue; fi
  for mode in default dyn; do
    if [ $mode = dyn ]; then dv=1; else dv=; fi
    out=$(cd tests/bridge && BREVI_DYN=$dv BREVI_PATH=1 timeout 600 ../../$BREVI run "$(basename "$f")" 2>/tmp/brevi_bridge_err)
    if grep -q '"path":"cpython"' /tmp/brevi_bridge_err || ! grep -q '"py":\[' /tmp/brevi_bridge_err; then
      echo "FAIL $f ($mode: not bridged)"; cat /tmp/brevi_bridge_err; fail=1
    elif [ "$out" != "$(cat "$exp")" ]; then
      echo "FAIL $f ($mode)"; cat /tmp/brevi_bridge_err; printf '%s\n' "$out" | diff "$exp" - ; fail=1
    else
      echo "ok   $f ($mode)"
    fi
  done
done
# A coroutine function handed to a library CPython runs would not be one of CPython's: the dynamic build refuses.
t=$(mktemp -d); cp -r tests/bridge/brlib "$t"
printf 'import asyncio\nfrom brlib import apply\nasync def f(x):\n    return x\nasync def main():\n    print(await asyncio.gather(*apply(f, [1, 2])))\nasyncio.run(main())\n' > "$t/m.py"
B=$BREVI; case $B in /*) ;; *) B=$PWD/$B ;; esac
o=$(cd "$t" && BREVI_DYN=1 $B run m.py 2>&1)
case "$o" in *"async code with a library CPython runs (brlib)"*) echo "ok   async code handing a coroutine to a bridged library is refused" ;; *) echo "FAIL async code handing a coroutine to a bridged library"; printf '%s\n' "$o"; fail=1 ;; esac
rm -rf "$t"
# Under the C locale (no LANG), text files a bridged library reads decode as UTF-8, as in python3 (#211).
f=tests/bridge/c_locale/read_utf8.py
out=$(env -u LANG -u LC_ALL -u LC_CTYPE $BREVI run --native "$f" 2>/dev/null)
if [ "$out" != "Mio 🖤" ]; then
  echo "FAIL $f"; echo "$out"; fail=1
else
  echo "ok   $f"
fi
#== sandbox split
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
#== limits
# --limit: the caller's resource limits. Each test's .flags holds them; the program's stdout, its exit status
# (124 when a limit stopped it) and the last stderr line, which names the limit, are compared.
for f in tests/limits/*.py; do
  exp="${f%.py}.expected"
  g=$(cat "${f%.py}.flags")
  case "$f" in */mem.py) [ "$(uname)" = Linux ] || continue ;; esac
  # threads needs a pids cgroup Brevi can create (refused elsewhere, checked after this loop).
  case "$f" in */threads.py) $BREVI run --limit threads=64 tests/limits/within.py >/dev/null 2>&1 || { echo "skip $f (no pids cgroup here)"; continue; } ;; esac
  out=$($BREVI run --sandbox $g "$f" 2>/tmp/brevi-limits-err; echo "exit $?"; tail -1 /tmp/brevi-limits-err)
  rm -f limits-*.out /tmp/brevi-limits-err
  if [ "$1" = "--update" ]; then printf '%s\n' "$out" > "$exp"; continue; fi
  if [ "$out" != "$(cat "$exp")" ]; then
    echo "FAIL $f"; printf '%s\n' "$out" | diff "$exp" - ; fail=1
  else
    echo "ok   $f"
  fi
done
# --limit threads runs where a pids cgroup can be made and is refused (exit 2) everywhere else, never ignored.
out=$($BREVI run --limit threads=64 tests/limits/within.py 2>&1 >/dev/null; echo "exit $?")
case "$out" in
  *"exit 0"|"--limit threads needs a cgroup with the kernel's pids controller"*"exit 2"|"--limit threads is enforced on Linux only"*"exit 2") echo "ok   threads limit held or refused" ;;
  *) echo "FAIL threads limit neither held nor refused: $out"; fail=1 ;;
esac
out=$($BREVI run --limit cpu=x tests/limits/within.py 2>&1; echo "exit $?"; $BREVI build --limit cpu=1 tests/limits/within.py 2>&1; echo "exit $?")
want='--limit takes cpu=SECS, mem=MB, time=SECS, output=KB, file=MB, written=MB, net=MB, threads=N, fds=N, files=N, comma separated (cpu=2,mem=256,time=5); '"'cpu=x'"' is not a number above 0
exit 2
--limit holds a program while it runs: use it with brevi run or brevi test (brevi verify takes it too)
exit 2'
if [ "$out" = "$want" ]; then echo "ok   --limit usage errors"; else echo "FAIL --limit usage errors"; printf '%s\n' "$out"; fail=1; fi
# written is counted by Brevi's own file layer: refused where code outside it could write (no sandbox, CPython).
out=$($BREVI run --limit written=1 tests/limits/within.py 2>&1 >/dev/null; echo "exit $?"; $BREVI run --sandbox --allow-py json --limit written=1 tests/limits/within.py 2>&1 >/dev/null; echo "exit $?")
case "$out" in
  *"need --sandbox without --allow native or --allow-py"*"exit 2"*"need --sandbox without"*"exit 2") echo "ok   --limit written refused outside Brevi's file layer" ;;
  *) echo "FAIL --limit written refused outside Brevi's file layer"; printf '%s\n' "$out"; fail=1 ;;
esac
# --sandbox: the program starts without the caller's environment (keys and tokens there stay out of reach).
out=$(SECRET_TEST_TOKEN=hunter2 $BREVI run --sandbox --allow read tests/sandbox_env/environ.py 2>&1)
if [ "$out" = "False" ] || [ "$(uname)" != Linux ]; then echo "ok   --sandbox clears the environment"; else echo "FAIL --sandbox clears the environment"; printf '%s\n' "$out"; fail=1; fi
#== exit split
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
#== bridge_sandbox
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
#== standalone split
# brevi build --standalone: a program calling Python libraries carries its own CPython. Run without python3 on
# PATH and, where user namespaces allow it, in an empty root holding only the C library.
for f in tests/standalone/*.py; do
  exp="${f%.py}.expected"
  t=$(mktemp -d)
  $BREVI build "$f" --native --standalone -o "$t/prog" >/dev/null 2>&1
  out=$(env -i HOME="$t" PATH=/nonexistent BREVI_PY_CACHE="$t/cache" "$t/prog" 2>&1; ls "$t"/cache/*/.brevi-ok >/dev/null 2>&1 || echo "bundle not unpacked")
  if unshare -r true 2>/dev/null; then
    # /dev/urandom too, as every machine has it: uv's CPython builds read it when they start.
    mkdir -p "$t/root/tmp" "$t/root/dev"; touch "$t/root/dev/urandom"; cp "$t/prog" "$t/root/prog"
    for l in $(ldd "$t/prog" | grep -o '/[^ ]*') $(ldconfig -p | grep -E '/(libm|libpthread|libdl|librt|libutil)\.so\.[0-9]+$' | grep -o '/[^ ]*$'); do mkdir -p "$t/root$(dirname "$l")"; cp -L "$l" "$t/root$l"; done
    bare=$(unshare -r -m sh -c 'mount --bind /dev/urandom "$1/dev/urandom" 2>/dev/null; exec env -i HOME=/tmp "$2" "$1" /prog' sh "$t/root" "$(command -v chroot)" 2>&1)
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
#== cli
# The command line, on both native builds: `brevi run FILE a b` hands `a b` to the program as python3 does (Brevi's
# own options may still follow FILE), a dashed word after FILE that is no Brevi option is refused (it may be a
# misspelt --sandbox) unless it follows --, a byte order mark may start a file, a `#` in an f-string's format spec
# is no comment, and an uncaught error on the typed build names its class.
for d in 0 1; do
  for c in "a b|a b" "a --native b|a --native b" "a -- --x|a -- --x" "-- --x a|--x a" "--fast a b|a b" "a b -- c|a b -- c"; do
    out=$(BREVI_DYN=$d $BREVI run tests/cli/argv.py --native ${c%%|*} 2>&1); want=$(python3 tests/cli/argv.py ${c#*|})
    if [ "$out" = "$want" ]; then echo "ok   cli args ($d: ${c%%|*})"; else echo "FAIL cli args ($d: ${c%%|*})"; echo "want $want"; echo "got  $out"; fail=1; fi
  done
  out=$(BREVI_DYN=$d $BREVI run tests/cli/argv.py --native --sandbx 2>&1; echo "exit $?")
  if printf '%s\n' "$out" | grep -q 'no option --sandbx' && printf '%s\n' "$out" | grep -qx 'exit 2'; then echo "ok   cli unknown option refused ($d)"; else echo "FAIL cli unknown option refused ($d)"; printf '%s\n' "$out"; fail=1; fi
  for f in fstring_spec bom; do
    out=$(BREVI_DYN=$d $BREVI run tests/cli/$f.py --native 2>&1); want=$(python3 tests/cli/$f.py 2>&1)
    if [ "$out" = "$want" ]; then echo "ok   cli $f ($d)"; else echo "FAIL cli $f ($d)"; printf '%s\n' "$want" | diff - "$(printf '%s\n' "$out" > /tmp/brevi_cli_$$; echo /tmp/brevi_cli_$$)"; fail=1; fi
  done
done
rm -f /tmp/brevi_cli_$$
out=$(BREVI_DYN=0 $BREVI run tests/cli/raised.py --native 2>&1; echo "exit $?")
want='before
{"e":"runtime","line":4,"type":"MyErr","msg":"custom failure"}
exit 1'
if [ "$out" = "$want" ]; then echo "ok   cli uncaught error type"; else echo "FAIL cli uncaught error type"; printf '%s\n' "$out"; fail=1; fi
# A value in an f-string that the typed build cannot read is refused there, never printed as text (it printed
# `{  1[1] }` for `f"{ {1: 2}[1] }"`); the default run builds it over dynamic values.
out=$(BREVI_DYN=0 $BREVI run tests/cli/fstring_brace_field.py --native 2>&1; echo "exit $?")
if printf '%s\n' "$out" | grep -q 'f-string value the typed build cannot read' && printf '%s\n' "$out" | grep -qx 'exit 1'; then echo "ok   cli f-string field refused typed"; else echo "FAIL cli f-string field refused typed"; printf '%s\n' "$out"; fail=1; fi
out=$($BREVI run tests/cli/fstring_brace_field.py --native 2>/dev/null)
if [ "$out" = "$(python3 tests/cli/fstring_brace_field.py)" ]; then echo "ok   cli f-string field"; else echo "FAIL cli f-string field"; printf '%s\n' "$out"; fail=1; fi
# A reader that stops early ends brevi report quietly (it panicked on the closed pipe).
t=$(mktemp -d); i=0
while [ $i -lt 400 ]; do mkdir -p "$t/differs/r$i"; printf '{"file":"x.py","on":"stdout"}\n' > "$t/differs/r$i/verdict.json"; i=$((i+1)); done
BREVI_CACHE="$t" $BREVI report 2>"$t/err" | head -1 >/dev/null
if [ ! -s "$t/err" ]; then echo "ok   cli report into a closed pipe"; else echo "FAIL cli report into a closed pipe"; cat "$t/err"; fail=1; fi
rm -rf "$t"
#== cache
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
# Tiered builds (src/tier.rs): run after an edit builds at opt-level 0 and builds opt-level 1 in the background;
# the next run of the unchanged file gets the opt-level 1 binary. build, --release and BREVI_TIERED=0 never tier.
t3=$(mktemp -d)
printf 'print(sum(i * i for i in range(10)))\n' > "$t3/main.py"
tr() { c=$1; shift; env BREVI_CACHE="$t3/cache" BREVI_TIME=1 BREVI_TIERED=1 $TENV $BREVI "$c" "$t3/main.py" "$@" 2>&1 | sed 's/^time .* ms  tiered:/tiered:/' | grep -v '^time '; }
t3out=$(tr run --native; i=0; while ls -a "$t3/cache/bin" | grep -q '^\.tmp-tier-' && [ $i -lt 600 ]; do sleep 0.1; i=$((i+1)); done
  tr run --native
  printf 'print(sum(i * i for i in range(11)))\n' > "$t3/main.py"
  tr build -o "$t3/out"; tr run --release; TENV=BREVI_TIERED=0 tr run; tr run --no-cache)
t3exp='tiered: building at opt-level 0, opt-level 1 in the background
285
tiered: the opt-level 1 binary took the quick one'"'"'s place
tiered: building at opt-level 0, opt-level 1 in the background
285
385
385
385'
if [ "$t3out" != "$t3exp" ]; then echo "FAIL tiered builds"; printf '%s\n' "$t3out" | diff - "$(printf '%s\n' "$t3exp" > "$t3/exp"; echo "$t3/exp")"; fail=1
else echo "ok   tiered builds"; fi
rm -rf "$t3"
# brevi verify: the native build against CPython on the same inputs. The certificate's timings, versions and
# path are left out (they change from machine to machine); the verdicts, cases and hashes must match.
v() { echo "-- $*"; o=$($BREVI verify "$@" 2>&1); c=$?; printf '%s\n' "$o" | sed -E 's/"(cpython_ms|brevi_ms|speedup)":[0-9.]+,?//g; s/"(brevi|python)":"[0-9.]+",?//g; s/"path":"[a-z]+",?//g; s/on two runs \(line [0-9]+: [^)]*\)/on two runs (line 1: ...)/; s/,}/}/'; echo "exit $c"; }
vout=$(v tests/verify/sq.py --input tests/verify/sq
  v tests/verify/argsum.py -- 3 4
  v tests/verify/clock.py
  v tests/verify/spin.py --once --timeout 1
  v tests/verify/spin.py --once --limit cpu=1
  v tests/verify/refused.py
  BREVI_PYTHON=tests/verify/fakepy.sh v tests/verify/sq.py
  v tests/verify/sq.py --sandbox
  v tests/run/arm_map.bv)
exp=tests/verify.expected
if [ "$1" = "--update" ]; then printf '%s\n' "$vout" > "$exp"
elif [ "$vout" != "$(cat "$exp")" ]; then echo "FAIL brevi verify"; printf '%s\n' "$vout" | diff "$exp" - ; fail=1
else echo "ok   brevi verify"; fi
# brevi run --verify: the run checked against CPython (docs/VERIFY_BY_DEFAULT.md). Same: the native output and a
# certificate on stderr; different: CPython's output and the first differing line, logged for brevi report.
rv_cache=$(mktemp -d)
rv() { echo "-- run --verify $*"; o=$(env BREVI_CACHE="$rv_cache" $BREVI run --verify "$@" 2>&1); c=$?; printf '%s\n' "$o" | grep -v '^{"path":' | sed -E 's/"(cpython_ms|brevi_ms|typed_ms|dynamic_ms|speedup)":[0-9.]+,?//g; s/"(brevi|cpython)":"[0-9]+\.[0-9]+\.[0-9]+",?//g; s/"path":"[a-z]+",?//g; s/"logged":"[^"]*",?//g; s/,}/}/; s/ [0-9]+ ms after/ N ms after/'; echo "exit $c"; }
rvout=$(printf '4\n' | rv tests/verify/sq.py
  printf '7\n' | rv tests/verify/sq.py
  rv tests/verify/argsum.py -- 3 4 </dev/null
  rv tests/verify/clock_ok.py </dev/null
  rv tests/verify/writes.py </dev/null
  rv tests/verify/refused.py </dev/null
  printf '2\n' | BREVI_PYTHON=tests/verify/fakepy.sh rv tests/verify/sq.py
  printf '2\n' | BREVI_PYTHON=tests/verify/fakepy.sh rv --strict tests/verify/sq.py
  printf '2\n' | BREVI_PYTHON=tests/verify/slowpy.sh rv tests/verify/sq.py
  rv --sandbox tests/verify/sq.py </dev/null
  rv --sandbox tests/verify/shared_sort.py </dev/null
  rv --sandbox tests/verify/argsum.py -- 3 4 </dev/null
  rv tests/run/arm_map.bv </dev/null
  echo "-- report"; env BREVI_CACHE="$rv_cache" $BREVI report | grep -c '^--- issue body'
  env BREVI_CACHE="$rv_cache" $BREVI report --clear; env BREVI_CACHE="$rv_cache" $BREVI report | sed 's/(.*)/(...)/')
rm -rf "$rv_cache"
exp=tests/run_verify.expected
if [ "$1" = "--update" ]; then printf '%s\n' "$rvout" > "$exp"
elif [ "$rvout" != "$(cat "$exp")" ]; then echo "FAIL brevi run --verify"; printf '%s\n' "$rvout" | diff "$exp" - ; fail=1
else echo "ok   brevi run --verify"; fi
# Only CPython 3.13 (src/cpython.rs): one of another version is refused by the fallback, verify and brevi-python,
# never run (tests/verify/py314.sh says it is 3.14).
o=$(BREVI_PYTHON=tests/verify/py314.sh $BREVI run tests/fallback/globals_write.py 2>&1
  BREVI_PYTHON=tests/verify/py314.sh $BREVI verify tests/verify/sq.py --input tests/verify/sq 2>&1
  echo 'print(6 * 7)' | BREVI_PYTHON=tests/verify/py314.sh $BREVI python - 2>&1)
n=$(printf '%s\n' "$o" | grep -c "is cpython 3.14.0, and Brevi answers as CPython 3.13 does")
if [ "$n" != 3 ] || printf '%s\n' "$o" | grep -qx 42; then echo "FAIL only CPython 3.13 ($n of 3 refused)"; printf '%s\n' "$o"; fail=1
else echo "ok   only CPython 3.13"; fi
exp=tests/cache.expected
if [ "$1" = "--update" ]; then printf '%s\n' "$out" > "$exp"
elif [ "$out" != "$(cat "$exp")" ]; then echo "FAIL result cache"; printf '%s\n' "$out" | diff "$exp" - ; fail=1
else echo "ok   result cache"; fi
# The same file built by several brevi processes at once (MCP sessions, parallel verify runs): each builds in its
# own folder, so none overwrites the generated Rust or the binary another is about to run. Before, they shared
# one folder per file and one could fail its build and fall back to CPython. --native makes such a clash an error.
# One typed file and one built over dynamic values. No folder is left behind once they have all exited.
t3=$(mktemp -d)
# (a tag that changes every run, so the binary cache has no build of them yet and all six run rustc at once)
tag=$(( $$ * 1000 + $(date +%s) % 1000 ))
printf 'total = %s\nfor i in range(1000):\n    total += i * i\nprint(total)\n' "$tag" > "$t3/typed.py"
printf 'def f(x):\n    return x if isinstance(x, int) else len(x)\nprint(sum(f(v) for v in [%s, "ab", 3, [4, 5]]))\n' "$tag" > "$t3/dyn.py"
for f in typed dyn; do
  for i in 1 2 3 4 5 6; do ($BREVI run --native "$t3/$f.py" > "$t3/$f.$i.out" 2> "$t3/$f.$i.err"; echo "exit $?" >> "$t3/$f.$i.out") & done
done
wait
t3bad=""
for i in 1 2 3 4 5 6; do
  [ "$(cat "$t3/typed.$i.out")" = "$(printf '%s\nexit 0' $((tag + 332833500)))" ] || t3bad="$t3bad typed.$i"
  [ "$(cat "$t3/dyn.$i.out")" = "$(printf '%s\nexit 0' $((tag + 7)))" ] || t3bad="$t3bad dyn.$i"
done
left=$(find "${TMPDIR:-/tmp}" -maxdepth 2 -path '*/brevi-*/p*' -newer "$t3/typed.py" 2>/dev/null | wc -l)
[ "$left" = 0 ] || t3bad="$t3bad (left $left build folders)"
if [ -n "$t3bad" ]; then
  echo "FAIL the same file built at once:$t3bad"; for o in "$t3"/*.out "$t3"/*.err; do echo "== $o"; cat "$o"; done; fail=1
else echo "ok   the same file built at once"; fi
rm -rf "$t3"
# A cached program is marked as used (the cache evicts the least recently used) without a handle open for
# writing: builds hard link it into place, and such a handle, however brief, makes any process starting one of
# its links fail with "Text file busy" (ETXTBSY, os error 26), as random tests did at 32 parallel jobs. While a
# link of it runs, a write handle cannot even be opened, so the mark must still land then.
t4=$(mktemp -d)
tag=$(( $$ * 1000 + $(date +%s) % 1000 ))
printf 'import sys\nprint(len(sys.stdin.read()) + %s)\n' "$tag" > "$t4/a.py"
cp "$t4/a.py" "$t4/b.py"; cp "$t4/a.py" "$t4/c.py"
t4bad=""
# a.py builds it; c.py, the same program, takes it from the binary cache: x1 is then a link of the cached binary
for f in a c; do
  BREVI_CACHE="$t4/cache" $BREVI build --native "$t4/$f.py" -o "$t4/x1" > "$t4/build-$f" 2>&1 || t4bad=" the build of $f.py failed"
done
t4c=$(find "$t4/cache" -samefile "$t4/x1" -path '*/bin/*' 2>/dev/null | head -1)
[ -n "$t4bad" ] || [ -n "$t4c" ] || t4bad=" no cached binary is linked to the build"
if [ -z "$t4bad" ]; then
  mkfifo "$t4/in"
  "$t4/x1" < "$t4/in" > "$t4/x1.out" &
  exec 9<> "$t4/in"  # read-write: never blocks, even if x1 failed to start
  touch -d '2001-01-01' "$t4/ref"; touch -d '2000-01-01' "$t4c"
  # b.py: the same program at another path, so the result cache misses and the binary cache answers
  BREVI_CACHE="$t4/cache" $BREVI build --native "$t4/b.py" -o "$t4/x2" > "$t4/build2" 2>&1 || t4bad=" second build failed"
  [ "$t4c" -nt "$t4/ref" ] || t4bad="$t4bad (the cached binary was not marked as used while it ran)"
  echo ab >&9; exec 9>&-; wait
  [ "$(cat "$t4/x1.out")" = "$((tag + 3))" ] || t4bad="$t4bad (the running program printed $(cat "$t4/x1.out"))"
fi
if [ -n "$t4bad" ]; then
  echo "FAIL the build cache marks a running program as used:$t4bad"; cat "$t4"/build* 2>/dev/null; fail=1
else echo "ok   the build cache marks a running program as used"; fi
rm -rf "$t4"
exit $fail

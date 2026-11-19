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
# Float matrix products, norms and linear algebra are Brevi's only where it models the OpenBLAS kernels numpy
# runs (see the numpy section). np_to_cpython FILE: FILE uses them and this CPU has no model, so Brevi must send it
# to CPython or refuse it (the numpy, bv_libs and sandbox sections use it).
np_flags() { for fl in "$@"; do grep -qw "$fl" /proc/cpuinfo 2>/dev/null || return 1; done; }
np_avx512=0; np_flags avx512f avx512dq avx512cd avx512bw avx512vl && np_avx512=1
np_fma=0; np_flags avx2 fma && np_fma=1
case "$(printf '%s' "${OPENBLAS_CORETYPE:-}" | tr A-Z a-z)" in
  "") np_kern=none; [ "$np_fma" = 1 ] && np_kern=hsw; [ "$np_avx512" = 1 ] && np_kern=skx ;;
  skylakex) np_kern=none; [ "$np_avx512" = 1 ] && np_kern=skx ;;
  haswell | zen) np_kern=none; [ "$np_fma" = 1 ] && [ "$np_avx512" = 0 ] && np_kern=hsw ;;
  *) np_kern=none ;;
esac
np_uses_blas() { grep -Eq '[])A-Za-z0-9_] *@=? |\.dot\(|np\.(dot|matmul|inner|vdot|cov|corrcoef|polyfit)\(|linalg' "$1"; }
np_to_cpython() {
  np_uses_blas "$1" || return 1
  [ "$np_kern" = none ] && return 0
  [ "$np_kern" = hsw ] && grep -q float32 "$1"
}
# Brevi names the CPython it handed a file to in its path report ("by"): python3 when that is the CPython 3.13 it
# found first on PATH, else the interpreter's path. With BREVI_PYTHON set (the next-Python lane gives Brevi uv's 3.13
# while python3 is the newer Python), that path is BREVI_PYTHON itself; by_python3 reads exactly that value as
# python3, so the .expected files hold on both lanes and any other interpreter named there still fails.
by_python3() {
  if [ -n "${BREVI_PYTHON:-}" ]; then
    by=$(printf '%s' "$BREVI_PYTHON" | sed 's/\\/\\\\/g; s/"/\\"/g; s/[][\\.*^$|]/\\&/g')
    sed "s|\"by\":\"$by\"|\"by\":\"python3\"|g"
  else
    cat
  fi
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
  out=$(printf '%s\n' "$out" | by_python3)
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
  out=$(printf '%s\n' "$out" | by_python3)
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
  out=$(printf '%s\n' "$out" | by_python3)
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
    if [ "$out" = "$("${BREVI_PYTHON:-python3}" "$it_dir/$p.py" 2>&1)" ]; then
      echo "ok   dynamic build subclasses an iterator type ($p)"
    else
      echo "FAIL dynamic build subclasses an iterator type ($p)"; printf '%s\n' "$out" | tail -3; fail=1
    fi
  done
  rm -rf "$it_dir"
fi
# A Python literal the typed lexer cannot read as written (bytes b"..." and rb"...", a u/F/rf string prefix, an
# imaginary 2j) is refused by the typed build with its own reason and built over dynamic values: rb"..." once
# built typed as a str and printed a\x00 for b'a\\x00'.
if [ "$1" != "--update" ]; then
  lt_dir=$(mktemp -d)
  printf 'print(rb"a\\x00", br"b")\n' > "$lt_dir/a.py"
  printf 'print(b"123\\x00", len(b"\\0"))\n' > "$lt_dir/b.py"
  printf 'x = 3\nprint(u"u", F"{x}", rf"{x}\\n")\n' > "$lt_dir/c.py"
  printf 'print(2j, 1.5J + 1, 1e3j)\n' > "$lt_dir/d.py"
  for p in "a:a bytes literal rb" "b:a bytes literal b" "c:the string prefix u" "d:a complex literal (2j)"; do
    n=${p%%:*}; why=${p#*:}
    err=$(BREVI_PATH=1 $BREVI build "$lt_dir/$n.py" -o "$lt_dir/prog" --no-cache 2>&1 >/dev/null)
    out=$("$lt_dir/prog" 2>&1)
    if printf '%s\n' "$err" | grep -q '^{"path":"dynamic".*'"$why" && [ "$out" = "$("${BREVI_PYTHON:-python3}" "$lt_dir/$n.py" 2>&1)" ]; then
      echo "ok   typed build refuses $why, the dynamic build prints what python3 prints"
    else
      echo "FAIL typed build refuses $why, the dynamic build prints what python3 prints"; printf '%s\n' "$err" "$out" | tail -3; fail=1
    fi
  done
  rm -rf "$lt_dir"
fi
# Flags the dynamic build cannot hold are refused when it builds, never answered wrong: an operator between members
# of two IntFlag classes (enum.py keeps the other class's member in the result), and a Flag whose __new__ shifts its
# values past what Brevi's flags hold.
if [ "$1" != "--update" ]; then
  fl_dir=$(mktemp -d)
  printf 'from enum import IntFlag, KEEP\nclass A(IntFlag, boundary=KEEP):\n    X = 1\nclass B(IntFlag):\n    Y = 2\nprint(repr(A.X | B.Y))\n' > "$fl_dir/a.py"
  printf 'from enum import Flag\nclass F(Flag):\n    def __new__(cls, c):\n        self = object.__new__(cls)\n        self._value_ = 1 << c\n        return self\n    a = 97\nprint(F.a.value)\n' > "$fl_dir/b.py"
  for p in "a:an operator between members of two different IntFlag classes" "b:a Flag whose __new__ makes its values with << or **"; do
    out=$(BREVI_DYN=1 $BREVI build "$fl_dir/${p%%:*}.py" -o "$fl_dir/prog" 2>&1)
    case "$out" in
      *"${p#*:}"*) echo "ok   dynamic build refuses ${p#*:}" ;;
      *) echo "FAIL dynamic build refuses ${p#*:}"; printf '%s\n' "$out" | tail -3; fail=1 ;;
    esac
  done
  rm -rf "$fl_dir"
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
  out=$(printf '%s\n' "$out" | by_python3)
  if [ "$1" = "--update" ]; then printf '%s\n' "$out" > "$exp"; continue; fi
  if [ "$out" != "$(cat "$exp")" ]; then
    echo "FAIL $f"; printf '%s\n' "$out" | diff "$exp" - ; fail=1
  else
    echo "ok   $f"
  fi
done
#== typed split
# Python programs built typed (BREVI_DYN=0 --native) must print what python3 prints, in time: the set
# tests take minutes if `in` scans a list instead of using AxSet's hash index. BREVI_GUARD=off: these test the
# typed build itself, also on files the guard sends to the dynamic build (tests/path checks the guard).
for f in tests/typed/*.py; do
  exp="${f%.py}.expected"
  inp="${f%.py}.in"; [ -f "$inp" ] || inp=/dev/null
  out=$(BREVI_DYN=0 BREVI_GUARD=off timeout 60 $BREVI run "$f" --native < "$inp" 2>&1)
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
  # Where this CPU has no OpenBLAS model, a file using float BLAS goes to CPython (CI has no numpy: only the path
  # line counts) or stops at the refused call with what it printed before matching.
  if np_to_cpython "$f" && grep -q '"path":"\(pybridge\|cpython\)"' /tmp/brevi_bv_err; then
    echo "ok   $f (CPython: no OpenBLAS model for this CPU)"
  elif [ "$out" != "$(cat "$exp")" ] && np_to_cpython "$f" && grep -q OpenBLAS /tmp/brevi_bv_err \
     && case "$(cat "$exp")" in "$out"*) true ;; *) false ;; esac; then
    echo "ok   $f (refused: no OpenBLAS model for this CPU)"
  elif grep -q '"path":"\(pybridge\|cpython\)"' /tmp/brevi_bv_err; then
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
  # No OpenBLAS model on this CPU: the build refuses the BLAS call (CPython would need a grant) and prints nothing else.
  if [ "$out" != "$(cat "${f%.bv}.expected")" ] && np_to_cpython "$f" && ! grep -q '"path":"\(pybridge\|cpython\)"' /tmp/brevi_bv_err \
     && printf '%s\n' "$out" | grep -q "CPython's .*numpy needs a grant" && ! printf '%s\n' "$out" | grep -qv '^{\|^note: '; then
    echo "ok   $f (--sandbox, refused: no OpenBLAS model for this CPU)"
  elif grep -q '"path":"\(pybridge\|cpython\)"' /tmp/brevi_bv_err || [ "$out" != "$(cat "${f%.bv}.expected")" ]; then
    echo "FAIL $f (--sandbox)"; cat /tmp/brevi_bv_err; fail=1
  else
    echo "ok   $f (--sandbox)"
  fi
done
#== numpy split
# numpy programs must print what CPython's numpy prints, byte for byte, with Brevi's own numpy: never CPython.
# The .expected files come from real numpy (numpy 2.5.5, see tools/upstream_pins.json; CI has none): `--update` needs it installed.
# A test that prints full precision can differ between numpy's AVX-512 and AVX2 kernels: it keeps the AVX2 output
# in NAME.avx2.expected, which a CPU without AVX-512 checks against.
# Float matrix products, norms and linear algebra are Brevi's only where it models the OpenBLAS kernels numpy
# runs (rt_np.rs np_blas): SkylakeX on AVX-512 CPUs, Haswell on AVX2+FMA ones (float64 only), with no
# OPENBLAS_CORETYPE naming other kernels. Elsewhere (ARM64, older x86-64) a test using them must go to CPython instead
# (only its path line is checked: CI has no numpy).
for f in tests/numpy/*.py; do
  exp="${f%.py}.expected"
  if [ "$1" = "--update" ]; then
    python3 -c "import numpy" 2>/dev/null && python3 "$f" > "$exp" 2>/dev/null
    [ -f "${f%.py}.avx2.expected" ] && OPENBLAS_CORETYPE=Haswell python3 "$f" > "${f%.py}.avx2.expected" 2>/dev/null
    continue
  fi
  [ "$np_kern" != skx ] && [ -f "${f%.py}.avx2.expected" ] && exp="${f%.py}.avx2.expected"
  # NAME_avx512.py: Brevi reproduces this numpy code for AVX-512 CPUs only; elsewhere the program goes to CPython
  case "$f" in *_avx512.py) [ "$np_avx512" = 0 ] && { echo "skip $f (needs AVX-512)"; continue; } ;; esac
  if np_to_cpython "$f"; then
    if BREVI_PATH=1 timeout 600 $BREVI run "$f" --no-cache 2>&1 >/dev/null | grep -q '^{"path":"\(pybridge\|cpython\)"'; then
      echo "ok   $f (CPython: no OpenBLAS model for this CPU)"
    else
      echo "FAIL $f (float matrix products must go to CPython on this CPU)"; fail=1
    fi
    continue
  fi
  out=$(BREVI_PATH=1 timeout 600 $BREVI run "$f" --native 2>/tmp/brevi_np_err)
  # A dynamic build that calls numpy through the bridge ("py":["numpy"]) runs CPython's numpy, not Brevi's.
  if ! grep -q '"path":"dynamic"' /tmp/brevi_np_err || grep '"path":"dynamic"' /tmp/brevi_np_err | grep -q '"py":\[[^]]*"numpy'; then
    echo "FAIL $f (not built with Brevi's numpy)"; cat /tmp/brevi_np_err; fail=1
  elif [ "$out" != "$(cat "$exp")" ]; then
    echo "FAIL $f"; printf '%s\n' "$out" | diff "$exp" - ; grep -v '"path"' /tmp/brevi_np_err | tail -5; fail=1
  else
    echo "ok   $f"
  fi
done
# Where the model exists, an OPENBLAS_CORETYPE naming other kernels stands in for an ARM64 CPU: the build sends
# the program to CPython rather than print numpy's last digits wrong.
if [ "$np_kern" != none ] && [ "$1" != "--update" ]; then
  if OPENBLAS_CORETYPE=Prescott BREVI_PATH=1 timeout 600 $BREVI run tests/numpy/lstsq.py --no-cache 2>&1 >/dev/null | grep -q '^{"path":"\(pybridge\|cpython\)"'; then
    echo "ok   numpy BLAS refusal"
  else
    echo "FAIL numpy BLAS refusal (lstsq.py built natively under OPENBLAS_CORETYPE=Prescott)"; fail=1
  fi
  # The result cache keys on what chose the path: a native build kept without OPENBLAS_CORETYPE is not replayed
  # under one naming other kernels, nor that CPython build once it is unset again (runcache.rs key).
  npc=$(mktemp -d); cp tests/numpy/lstsq.py "$npc/main.py"
  npp() { env BREVI_CACHE="$npc/cache" BREVI_PATH=1 "$@" timeout 600 $BREVI run "$npc/main.py" 2>&1 >/dev/null | grep -o '^{"path":"[a-z]*"' | head -1; }
  npc_out="$(npp; npp OPENBLAS_CORETYPE=Prescott; npp)"
  if [ "$npc_out" = '{"path":"dynamic"
{"path":"pybridge"
{"path":"dynamic"' ] || [ "$npc_out" = '{"path":"dynamic"
{"path":"cpython"
{"path":"dynamic"' ]; then
    echo "ok   result cache keys on OPENBLAS_CORETYPE"
  else
    echo "FAIL result cache keys on OPENBLAS_CORETYPE (path lines: no env, Prescott, no env)"; printf '%s\n' "$npc_out"; fail=1
  fi
  rm -rf "$npc"
fi
# numpy's own settings (NPY_DISABLE_CPU_FEATURES, NPY_ENABLE_CPU_FEATURES and any other NPY_* / NUMPY_*) change
# which SIMD loops numpy runs, so its float32 exp/tanh/... digits: with one set the build uses CPython's numpy
# (dynpy/mod.rs to_rust, emit::np_settings), and the result cache keys on them, so a native build kept without
# them is not replayed under them, nor the CPython build once they are unset (runcache.rs key).
if [ "$1" != "--update" ]; then
  npf=tests/numpy/compound_interest.py; [ "$np_avx512" = 1 ] && npf=tests/numpy/float32_ufuncs_avx512.py
  npc=$(mktemp -d); cp "$npf" "$npc/main.py"
  # Brevi's numpy: path dynamic with no "py" list naming numpy; CPython's: path pybridge or cpython, or a dynamic
  # build calling numpy through the bridge ("py":["numpy"]).
  npp() {
    l=$(env BREVI_CACHE="$npc/cache" BREVI_PATH=1 "$@" timeout 600 $BREVI run "$npc/main.py" 2>&1 >/dev/null | grep '^{"path":' | head -1)
    case "$l" in '{"path":"dynamic"'*) case "$l" in *'"py":['*'"numpy'*) echo cpython ;; *) echo native ;; esac ;; '{"path":"pybridge"'* | '{"path":"cpython"'*) echo cpython ;; *) echo "other: $l" ;; esac
  }
  npc_out="$(npp; npp NPY_DISABLE_CPU_FEATURES=AVX512_SKX; npp "NPY_ENABLE_CPU_FEATURES=SSE SSE2 SSE3"; npp NPY_DISABLE_CPU_FEATURES=; npp)"
  if [ "$(echo $npc_out)" = "native cpython cpython cpython native" ]; then
    echo "ok   numpy CPU feature settings go to CPython (and key the result cache)"
  else
    echo "FAIL numpy CPU feature settings (path lines: none, DISABLE, ENABLE, DISABLE empty, none)"; printf '%s\n' "$npc_out"; fail=1
  fi
  rm -rf "$npc"
fi
# A binary built here and copied to a CPU without a modelled kernel must raise NotImplementedError at each float
# BLAS/LAPACK call (rt_np.rs np_blas_ok), never print other digits. qemu stands in for that CPU: SandyBridge (no AVX2
# or FMA) for the float64 kernels, Haswell (no AVX-512) for the float32 ones. Where this system's own libraries need
# a newer CPU (GitHub's Ubuntu 26.04 runner: /bin/true stops with an illegal instruction on qemu's SandyBridge), the
# binary runs on an ubuntu:24.04 container's libraries instead (docker, with this qemu if it is static).
# BREVI_QEMU_TESTS=1 (GitHub CI) makes a skip a failure.
if [ "$np_kern" != none ] && [ "$1" != "--update" ] && [ "$(uname -m)" = x86_64 ]; then
  if ! command -v qemu-x86_64 >/dev/null; then
    if [ "${BREVI_QEMU_TESTS:-}" = 1 ]; then echo "FAIL numpy runtime refusal (qemu-x86_64 not installed)"; fail=1
    else echo "skip numpy runtime refusal (no qemu-x86_64)"; fi
  else
    npq=$(mktemp -d)
    npq_run() { # npq_run CPU PROGRAM: PROGRAM on qemu's CPU, on this system's libraries or else ubuntu:24.04's
      if qemu-x86_64 -cpu "$1" /bin/true 2>/dev/null; then qemu-x86_64 -cpu "$1" "$2"
      else docker run --rm -v "$npq:$npq:ro" -v "$(command -v qemu-x86_64):/usr/local/bin/qemu-x86_64:ro" ubuntu:24.04 qemu-x86_64 -cpu "$1" "$2"; fi
    }
    for t in blas_refusal:SandyBridge f32_refusal:Haswell; do
      f=tests/numpy_cpu/${t%%:*}.py cpu=${t##*:}
      # float32 products are Brevi's only on AVX-512 CPUs: elsewhere the build already sends them to CPython
      [ "$cpu" = Haswell ] && [ "$np_kern" != skx ] && { echo "skip $f (needs AVX-512 to build natively)"; continue; }
      if ! npq_run "$cpu" /bin/true >/dev/null 2>&1; then
        echo "FAIL $f (no system runs on qemu's $cpu CPU here: docker with ubuntu:24.04 and a static qemu-x86_64 are needed)"; fail=1
      elif ! BREVI_PATH=1 timeout 600 $BREVI build "$f" -o "$npq/b" --no-cache 2>"$npq/err" >/dev/null || ! grep -q '"path":"dynamic"' "$npq/err"; then
        echo "FAIL $f (not built with Brevi's numpy)"; cat "$npq/err"; fail=1
      elif [ "$(npq_run "$cpu" "$npq/b" 2>/dev/null)" != "$(cat "${f%.py}.expected")" ]; then
        echo "FAIL $f (on a $cpu CPU)"; npq_run "$cpu" "$npq/b" 2>&1 | diff "${f%.py}.expected" - | head -20; fail=1
      else
        echo "ok   $f (refused on a $cpu CPU)"
      fi
    done
    rm -rf "$npq"
  fi
fi

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
# onnxruntime programs must print what CPython's onnxruntime 1.32 prints (1.29 and 1.31 print the same), with Brevi's own onnxruntime over ONNX
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
# anthropic and openai programs must print what CPython's anthropic 2.8 and openai 3.51 print (models, streams,
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
# A namedtuple field name NFKC normalization may change stops the program, even inside try/except: CPython compiles
# __new__ from the normalized names ('\ufb01' and 'fi' collide there), so Brevi refuses rather than answer differently.
printf 'from collections import namedtuple\ntry:\n    T = namedtuple("T", ["\\ufb01"])\n    print(T._fields)\nexcept Exception as e:\n    print("caught", e)\n' > "$t/nt_nfkc.py"
o=$(BREVI_DYN=1 $BREVI run "$t/nt_nfkc.py" 2>&1)
case "$o" in "NotImplementedError: brevi: a namedtuple field name NFKC normalization may change ('"*"') is not supported in dynamic builds") echo "ok   a namedtuple field name NFKC may change stops" ;; *) echo "FAIL a namedtuple field name NFKC may change"; printf '%s\n' "$o"; fail=1 ;; esac
# pickle.loads gives the program's objects their state after CPython's Unpickler has finished: a BUILD that runs
# the program's code (its own __setstate__) stops the program, even inside try/except, when CPython would run it
# before code of the program the load ran (a REDUCE of a program function), on an object the result does not
# reach, or before the load fails (tests/dyn/pickle_setstate_order.py has the orders that match).
ps_hdr='import pickle\nclass A:\n    def __setstate__(self, s):\n        print("setstate")\n        self.__dict__.update(s)\ndef f(x):\n    print("f", x)\n    return x\n'
printf "$ps_hdr"'print(pickle.loads(b"\\x80\\x02]q\\x00(c__main__\\nA\\nq\\x01)\\x81q\\x02}q\\x03X\\x01\\x00\\x00\\x00vq\\x04K\\x01sbc__main__\\nf\\nq\\x05X\\x01\\x00\\x00\\x00xq\\x06\\x85q\\x07Rq\\x08e."))\n' > "$t/ps1.py"
printf "$ps_hdr"'print(pickle.loads(b"\\x80\\x02c__main__\\nA\\n)\\x81}b0N."))\n' > "$t/ps2.py"
printf "$ps_hdr"'try:\n    pickle.loads(b"\\x80\\x02c__main__\\nA\\n)\\x81}b")\nexcept Exception as e:\n    print(type(e).__name__)\n' > "$t/ps3.py"
for c in "ps1:in another order than CPython's" "ps2:where the result does not reach it" "ps3:where the result does not reach it or the load fails after it"; do
    o=$(BREVI_DYN=1 $BREVI run "$t/${c%%:*}.py" 2>&1)
    case "$o" in *"NotImplementedError: brevi: loading a pickle"*"${c#*:}"*" is not supported in dynamic builds") echo "ok   a pickle BUILD running program code out of CPython's order stops (${c%%:*})" ;; *) echo "FAIL a pickle BUILD running program code out of CPython's order (${c%%:*})"; printf '%s\n' "$o"; fail=1 ;; esac
done
# Every other refusal on the pickle and copy paths stops the program too, as no `except` may catch it and go on
# with an answer CPython never gives: a __reduce__ giving a global name, a bound method, items added to a program
# instance by a load (the Python glue's refusals), an int subclass made from a list the load adds to after it. A copy whose __reduce__ gives a tuple of the wrong length is CPython's
# TypeError, not a refusal.
pk_try='try:\n    %s\nexcept Exception as e:\n    print("caught", type(e).__name__)\nprint("after")\n'
printf "import pickle\nclass A:\n    def __reduce__(self):\n        return 'a'\na = A()\n$pk_try" 'print(pickle.dumps(A()))' > "$t/pk1.py"
printf "import pickle\nclass T:\n    def k(self):\n        return 1\n$pk_try" 'print(pickle.dumps(T().k))' > "$t/pk2.py"
printf "import pickle\nclass A:\n    pass\n$pk_try" 'print(pickle.loads(b"\x80\x02c__main__\nA\n)\x81(K\x01e."))' > "$t/pk3.py"
printf "import pickle\nclass N(int):\n    pass\nx = []\nn = N(3)\nn.owner = x\nx.append(n)\n$pk_try" 'print(pickle.loads(pickle.dumps(x, 2)))' > "$t/pk5.py"
for c in "pk1:pickling an object of the program whose __reduce__ gives a global name" "pk2:pickling a bound method" "pk3:loading a pickle that adds items to an instance of a class of the program" "pk5:loading a pickle whose object of the program is made from a list, dict or set the load adds to after it is made"; do
    o=$(BREVI_DYN=1 $BREVI run "$t/${c%%:*}.py" 2>&1)
    case "$o" in "NotImplementedError: brevi: ${c#*:} is not supported in dynamic builds") echo "ok   a pickle refusal inside try/except stops (${c%%:*})" ;; *) echo "FAIL a pickle refusal inside try/except (${c%%:*})"; printf '%s\n' "$o"; fail=1 ;; esac
done
printf 'import copy\nclass A:\n    def __reduce__(self):\n        return (A, (), None, None, None, None)\nfor f in (copy.copy, copy.deepcopy):\n    try:\n        f(A())\n    except TypeError as e:\n        print(e)\nclass B:\n    def __reduce__(self):\n        return (B,)\ntry:\n    copy.copy(B())\nexcept TypeError as e:\n    print(e)\n' > "$t/pk4.py"
o=$(BREVI_DYN=1 $BREVI run "$t/pk4.py" 2>&1)
if [ "$o" = "$("${BREVI_PYTHON:-python3}" "$t/pk4.py" 2>&1)" ]; then echo "ok   copy of a __reduce__ tuple of the wrong length is CPython's TypeError"; else echo "FAIL copy of a __reduce__ tuple of the wrong length"; printf '%s\n' "$o"; fail=1; fi
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
o=$(BREVI_DYN=1 $BREVI build "$t/m3.py" -o "$t/m3" 2>&1 && "$t/m3" 2>&1); w=$("${BREVI_PYTHON:-python3}" "$t/m3.py" 2>&1)
if [ "$o" = "$w" ]; then echo "ok   a library module's repr is CPython's"; else echo "FAIL a library module's repr"; printf '%s\n' "$o" "$w"; fail=1; fi
printf 'import sys\nprint(f"{sys.stdout}")\n' > "$t/m4.py"
o=$(BREVI_DYN=1 $BREVI build "$t/m4.py" -o "$t/m4" 2>&1)
case "$o" in *"the repr of sys.stdout is not supported in dynamic builds"*) echo "ok   the repr of sys.stdout is refused when built" ;; *) echo "FAIL the repr of sys.stdout when built"; printf '%s\n' "$o"; fail=1 ;; esac
printf 'import decimal\nc = [decimal.getcontext()]\nprint(c)\n' > "$t/m5.py"
o=$(BREVI_DYN=1 $BREVI run "$t/m5.py" 2>&1)
case "$o" in *"NotImplementedError: brevi: the repr of decimal.context is not supported in dynamic builds") echo "ok   a decimal context's repr the build cannot see stops" ;; *) echo "FAIL a decimal context's repr at run time"; printf '%s\n' "$o"; fail=1 ;; esac
# A missing attribute of a program file that shadows a standard library module is CPython 3.13's AttributeError
# (it names the file); `from FILE import *` of a program file, and a new attribute set through a program
# module object, are refused (the file's names are fixed when it is built).
printf 'X = 1\n' > "$t/tabnanny.py"
printf 'import tabnanny\ntry:\n    tabnanny.nope\nexcept AttributeError as e:\n    print(e)\n' > "$t/m6.py"
o=$(BREVI_DYN=1 $BREVI run "$t/m6.py" 2>&1)
if [ "$o" = "$("${BREVI_PYTHON:-python3}" "$t/m6.py" 2>&1)" ]; then echo "ok   a missing attribute of a file shadowing the stdlib"; else echo "FAIL a missing attribute of a file shadowing the stdlib"; printf '%s\n' "$o"; fail=1; fi
printf 'from helper import *\nprint(X)\n' > "$t/m7.py"
o=$(BREVI_DYN=1 $BREVI run "$t/m7.py" 2>&1)
case "$o" in *"from helper import * (a file of the program) is not supported in dynamic builds"*) echo "ok   a star import of a program file is refused" ;; *) echo "FAIL a star import of a program file"; printf '%s\n' "$o"; fail=1 ;; esac
printf 'import helper\nm = helper\nm.X = 2\nprint(helper.X)\nm.Z = 3\n' > "$t/m8.py"
o=$(BREVI_DYN=1 $BREVI run "$t/m8.py" 2>&1)
case "$o" in "2"*"NotImplementedError: brevi: setting a new attribute helper.Z through the module object is not supported in dynamic builds") echo "ok   a new attribute through a program module object stops" ;; *) echo "FAIL a new attribute through a program module object"; printf '%s\n' "$o"; fail=1 ;; esac
printf 'import json\ntry:\n    print(json.loads(chr(34) + chr(0xd800) + chr(34)))\nexcept BaseException:\n    print("caught")\nprint("after")\n' > "$t/s.py"
o=$(BREVI_DYN=1 $BREVI run "$t/s.py" 2>&1); c=$?
case "$c:$o" in "1:NotImplementedError: brevi: this operation on a str with a lone surrogate (U+D800..U+DFFF) is not supported in dynamic builds yet"*) echo "ok   a lone surrogate on an unwritten path stops the program" ;; *) echo "FAIL a lone surrogate on an unwritten path ($c)"; printf '%s\n' "$o"; fail=1 ;; esac
# sys.modules: a read whose CPython answer is not sure is refused when it builds (tests/dyn/sys_modules_startup.py
# has the exact ones).
for c in 'import sys\nprint(len(sys.modules))\n|only as `k in sys.modules`' \
         'import sys, json\nprint("re" in sys.modules)\n|when the program imports json' \
         'import sys\nprint("encodings.utf_8" in sys.modules)\n|depends on its environment' \
         'import sys\nprint("x".encode("idna"), "re" in sys.modules)\n|look up a codec by name' \
         'from sys import modules\n|from sys import modules is not supported' \
         'import sys\nprint(sys.modules["os"].sep)\n|a module CPython loads at startup'; do
  printf "${c%%|*}" > "$t/sm.py"
  o=$(BREVI_DYN=1 $BREVI build "$t/sm.py" -o "$t/sm" 2>&1)
  case "$o" in *"${c#*|}"*) echo "ok   sys.modules refused: ${c#*|}" ;; *) echo "FAIL sys.modules not refused: ${c#*|}"; printf '%s\n' "$o"; fail=1 ;; esac
done
# a key known only at run time is refused when it builds, never at run time (even when every import is quiet),
# unless it is a parameter whose every call passes a literal (tests/dyn/sys_modules_param.py)
for c in 'import sys, json\nk = "re"\nprint(k in sys.modules)\n' 'import sys\nfor k in ["os", "x"]:\n    print(sys.modules.get(k))\n' \
         'import sys\ndef mod(n):\n    return sys.modules[n]\nprint(mod("sys"), mod("x" * 2))\n' 'import sys\ndef mod(n):\n    return sys.modules[n]\ng = mod\nprint(mod("sys"))\n' \
         'import sys\ndef mod(n):\n    return sys.modules[n]\nprint(mod("sys"), globals())\n' 'import sys\ndef mod(n):\n    return sys.modules[n]\nprint(mod("__main__"))\n' \
         'import sys\ndef mod(n):\n    n = "os"\n    return sys.modules[n]\nprint(mod("sys"))\n' 'import sys\ndef mod(n):\n    return sys.modules[n]\nprint(mod(*["sys"]))\n' \
         'import sys\nprint(1)\nimport json, functools\nprint(sys.modules["json"] is json)\n'; do
  printf "$c" > "$t/sm.py"
  o=$(BREVI_DYN=1 $BREVI build "$t/sm.py" -o "$t/sm" 2>&1)
  case "$o" in *"is not supported in dynamic builds"*) echo "ok   sys.modules key refused at build: $(printf "$c" | tail -n 1)" ;; *) echo "FAIL sys.modules key not refused at build"; printf '%s\n' "$o"; fail=1 ;; esac
done
# globals() by a key known only at run time, Enum._convert_ and enum.global_enum (tests/dyn/enum_convert_global.py
# has the answers): a store that would change a name the build fixed, add one the program reads by name, or a
# _convert_ whose filter or values the build cannot follow stops the program; under __del__ it is refused when built.
for c in 'def f():\n    pass\nk = "f"\nglobals()[k] = 1\n|rebinding '"'"'f'"'"' (bound by a def, class or import' \
         'import os\nk = "os"\nglobals()[k] = 1\n|rebinding '"'"'os'"'"'' \
         'k = "zz"\nglobals()[k] = 1\nprint(zz)\n|adding the global '"'"'zz'"'"' by key, which the program reads by name' \
         'k = "__doc__"\nglobals()[k] = 1\n|globals() store to '"'"'__doc__'"'"'' \
         'import enum\nA_X = 1\nE = enum.Enum._convert_("E", __name__, filter=lambda n: n.startswith("__"))\n|a filter that takes the module'"'"'s own dunder names' \
         'import enum\nclass P:\n    pass\nA_X = P()\nE = enum.Enum._convert_("E", __name__, filter=lambda n: n.startswith("A_"))\n|whose value'"'"'s comparisons the build cannot follow' \
         'import enum\nA_X = 1\nE = enum.Flag._convert_("E", __name__, filter=lambda n: n.startswith("A_"))\n|_convert_ on a Flag' \
         'import enum\nA_X = 1\nE = enum.Enum._convert_("E", __name__, filter=lambda n: print(n) or n.startswith("A_"))\n|EnumType._convert_ read through an Enum class' \
         'import enum\n@enum.global_enum\nclass C(enum.Enum):\n    A = 1\nprint(A)\n|adding the global '"'"'A'"'"' by key'; do
  printf "${c%%|*}" > "$t/gk.py"
  o=$(BREVI_DYN=1 $BREVI run "$t/gk.py" 2>&1)
  case "$o" in *"NotImplementedError: brevi: "*"${c#*|}"*) echo "ok   globals by key stops: ${c#*|}" ;; *) echo "FAIL globals by key does not stop: ${c#*|}"; printf '%s\n' "$o"; fail=1 ;; esac
done
printf 'class D:\n    def __del__(self):\n        pass\nk = "x"\nglobals()[k] = 1\n' > "$t/gk.py"
o=$(BREVI_DYN=1 $BREVI build "$t/gk.py" -o "$t/gk" 2>&1)
case "$o" in *"globals()[key] = value is not supported in dynamic builds in a program with __del__"*) echo "ok   globals by key under __del__ is refused" ;; *) echo "FAIL globals by key under __del__"; printf '%s\n' "$o"; fail=1 ;; esac
# A unittest.mock object stored on a class of the program (tests/dyn/mock_on_class.py has the answers) builds only
# under a plain name or as __set__/__delete__/__set_name__/__get__, of a Mock or MagicMock, in a program that reads
# nothing showing what a mock was called with; one stored under another special name where the build cannot see it
# stops when stored, and Python using a sealed object of the program (a repr of call_args) stops the program.
for c in 'from unittest.mock import Mock\nclass D:\n    pass\nD.__len__ = Mock(return_value=3)\nprint(len(D()))\n|a Mock stored as __len__' \
         'from unittest.mock import Mock\nclass D:\n    pass\nD.__set__ = Mock()\nclass C:\n    x = D()\nC().x = 1\nprint(D.__set__.call_args)\n|call_args of a program with a Mock' \
         'from unittest.mock import PropertyMock\nclass D:\n    pass\nD.p = PropertyMock(return_value=3)\nprint(D().p)\n|PropertyMock stored on a class' \
         'from unittest.mock import Mock\nclass E:\n    _missing_ = Mock()\nprint(1)\n|a Mock stored as _missing_' \
         'from unittest.mock import Mock\nclass D:\n    pass\nsetattr(D, "__s" + "et__", Mock())\n|under a name the build cannot see'; do
  printf "${c%%|*}" > "$t/mk.py"
  o=$(BREVI_DYN=1 $BREVI build "$t/mk.py" -o "$t/mk" 2>&1)
  case "$o" in *"${c#*|}"*"needs CPython"*) echo "ok   mock on a class refused at build: ${c#*|}" ;; *) echo "FAIL mock on a class not refused at build: ${c#*|}"; printf '%s\n' "$o"; fail=1 ;; esac
done
for c in 'from unittest.mock import Mock\nclass D:\n    pass\nm = Mock(return_value=3)\nD.__len__ = m\nprint(len(D()))\n|stored as __len__ of a class of the program' \
         'from unittest.mock import Mock\nclass D:\n    pass\nm = Mock()\nm(D())\ntry:\n    print(m.call_args)\nexcept Exception as e:\n    print("caught", e)\n|a Python library using an object of the program that a unittest.mock object was called with'; do
  printf "${c%%|*}" > "$t/mk.py"
  o=$(BREVI_DYN=1 $BREVI run "$t/mk.py" 2>&1)
  case "$o" in "NotImplementedError: brevi: "*"${c#*|}"*) echo "ok   mock on a class stops: ${c#*|}" ;; *) echo "FAIL mock on a class does not stop: ${c#*|}"; printf '%s\n' "$o"; fail=1 ;; esac
done
# Brevi's own inspect has signature, Signature.parameters and Parameter (tests/dyn/inspect_signature.py has the
# answers); constructing a Signature or a Parameter builds with CPython's inspect, whose signature cannot read the
# program's defs (refused), and what a Parameter does not have stops the program.
for c in 'from inspect import signature, Parameter\ndef f(a): pass\nprint(Parameter("a", Parameter.POSITIONAL_ONLY), signature(f))\n|inspect.signature' \
         'import inspect\ndef f(a): pass\nprint(inspect.Signature([]), inspect.signature(f))\n|inspect.signature'; do
  printf "${c%%|*}" > "$t/sg.py"
  o=$(BREVI_DYN=1 $BREVI build "$t/sg.py" -o "$t/sg" 2>&1)
  case "$o" in *"${c#*|}"*"needs CPython"*) echo "ok   inspect.Signature/Parameter construction refused at build" ;; *) echo "FAIL inspect.Signature/Parameter construction not refused at build"; printf '%s\n' "$o"; fail=1 ;; esac
done
for c in 'import inspect\ndef f(a): pass\nprint(inspect.signature(f).parameters["a"].replace(name="b"))\n|inspect.Parameter.replace' \
         'import inspect\ndef f(a): pass\nprint(hash(inspect.signature(f).parameters["a"]))\n|hash() of an inspect.Parameter' \
         'import inspect\nclass K:\n    pass\nprint(inspect.signature(K))\n|of a '"'"'type'"'"' object'; do
  printf "${c%%|*}" > "$t/sg.py"
  o=$(BREVI_DYN=1 $BREVI run "$t/sg.py" 2>&1)
  case "$o" in *"NotImplementedError: brevi: "*"${c#*|}"*) echo "ok   inspect.signature stops: ${c#*|}" ;; *) echo "FAIL inspect.signature does not stop: ${c#*|}"; printf '%s\n' "$o"; fail=1 ;; esac
done
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
want=$(env -u LANG -u LC_ALL -u LC_CTYPE -u PYTHONIOENCODING -u PYTHONUTF8 "${BREVI_PYTHON:-python3}" "$t/c.py" 2>&1 | od -An -tx1)
got=$(env -u LANG -u LC_ALL -u LC_CTYPE -u PYTHONIOENCODING -u PYTHONUTF8 BREVI_DYN=1 $BREVI run "$t/c.py" 2>&1 | od -An -tx1)
if [ -n "$want" ] && [ "$got" = "$want" ]; then echo "ok   a lone surrogate on stdout under the C locale"; else echo "FAIL a lone surrogate on stdout under the C locale"; echo "want $want"; echo "got  $got"; fail=1; fi
# Under --sandbox the program reads no environment, so it cannot name stdout's error handler: it stops instead.
o=$(BREVI_DYN=1 $BREVI run --sandbox "$t/c.py" 2>&1); c=$?
case "$c:$o" in "1:NotImplementedError: brevi: this operation on a str with a lone surrogate"*) echo "ok   a lone surrogate on stdout under --sandbox stops" ;; *) echo "FAIL a lone surrogate on stdout under --sandbox ($c)"; printf '%s\n' "$o"; fail=1 ;; esac
# re.LOCALE on bytes asks the C library in the environment's LC_CTYPE locale, as CPython's sre does (#589: Windows'
# cp1252 locale). Compared with python3 under single-byte locales built here (skipped where localedef cannot).
printf 'import re\nfor b in range(256):\n    c = bytes([b])\n    print(b, bool(re.match(re.escape(c), c.lower(), re.L | re.I)), bool(re.match(re.escape(c), c.upper(), re.L | re.I)), bool(re.match(rb"\\w", c, re.L)), bool(re.match(b"[" + re.escape(c) + b"]", c.swapcase(), re.L | re.I)), bool(re.match(rb"(.)\\1", c + c.swapcase(), re.L | re.I)), len(re.findall(rb"\\b", b"a" + c + b" ", re.L)))\nprint(re.findall(rb"(?Li)[\\xc0-\\xc5]+|\\w+", b"\\xe0\\xe5\\xc0 ab\\xff_"))\n' > "$t/loc.py"
mkdir -p "$t/locs"
for loc in "ISO-8859-1 en_US" "CP1252 en_US" "ISO-8859-9 tr_TR"; do
  cm=${loc% *}; li=${loc#* }
  if localedef -f "$cm" -i "$li" "$t/locs/$li.$cm" >/dev/null 2>&1 || [ -d "$t/locs/$li.$cm" ]; then
    want=$(LOCPATH="$t/locs" LC_ALL="$li.$cm" "${BREVI_PYTHON:-python3}" "$t/loc.py" 2>&1)
    got=$(LOCPATH="$t/locs" LC_ALL="$li.$cm" BREVI_DYN=1 $BREVI run "$t/loc.py" 2>/dev/null)
    if [ -n "$want" ] && [ "$got" = "$want" ]; then echo "ok   re.LOCALE under $li.$cm"; else echo "FAIL re.LOCALE under $li.$cm"; printf 'want %s\ngot  %s\n' "$(printf '%s\n' "$want" | head -3)" "$(printf '%s\n' "$got" | head -3)"; fail=1; fi
  else
    echo "skip re.LOCALE under $li.$cm (localedef cannot build it here)"
  fi
done
# Under --sandbox the program reads no environment, so it cannot know the locale re.LOCALE asks: it stops instead.
printf 'import re\nprint(re.match(b"a", b"A", re.L | re.I))\n' > "$t/locsb.py"
o=$(BREVI_DYN=1 $BREVI run --sandbox "$t/locsb.py" 2>&1); c=$?
case "$c:$o" in *"1:NotImplementedError: brevi: re.LOCALE under --sandbox"*) echo "ok   re.LOCALE under --sandbox stops" ;; *) echo "FAIL re.LOCALE under --sandbox ($c)"; printf '%s\n' "$o"; fail=1 ;; esac
# UTF-8 mode (PYTHONUTF8=1) reaches the embedded CPython, which Py_InitializeEx alone ignores: the preferred
# encoding is 'utf-8' as in python3, not the locale's (ASCII under C here, the ANSI code page on Windows; #589).
printf 'import locale\nprint(locale.getpreferredencoding())\n' > "$t/penc.py"
want=$(LC_ALL=C PYTHONUTF8=1 "${BREVI_PYTHON:-python3}" "$t/penc.py" 2>&1)
got=$(LC_ALL=C PYTHONUTF8=1 BREVI_DYN=1 $BREVI run "$t/penc.py" 2>&1)
if [ "$want" = utf-8 ] && [ "$got" = "$want" ]; then echo "ok   PYTHONUTF8=1 preferred encoding"; else echo "FAIL PYTHONUTF8=1 preferred encoding: want $want got $got"; fail=1; fi
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
# A compiled function has no CPython bytecode: a bridged library that reads it (dis loaded through importlib,
# inspect.signature reading co_argcount) gets the dynamic build's refusal, never a TypeError CPython would not raise.
t=$(mktemp -d)
printf 'import importlib\ndis = importlib.import_module("dis")\ndef f(x):\n    return x\ntry:\n    print(list(dis.get_instructions(f)))\nexcept TypeError as e:\n    print("TypeError", e)\n' > "$t/via_dis.py"
printf 'import importlib\ndef f(x, y=2):\n    return x\nprint(importlib.import_module("inspect").signature(f))\n' > "$t/via_sig.py"
for c in "dis:brevi: the code object's co_code" "sig:brevi: the code object's co_argcount"; do
  n=${c%%:*}; want=${c#*:}
  o=$(cd "$t" && BREVI_DYN=1 $B run via_$n.py 2>&1)
  case "$o" in *"$want"*) echo "ok   a bridged library reading a compiled function's bytecode is refused ($n)" ;; *) echo "FAIL a bridged library reading a compiled function's bytecode is refused ($n)"; printf '%s\n' "$o" | tail -3; fail=1 ;; esac
done
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
  if [ "$out" != "$(cat "$exp")" ] && np_to_cpython "$f" && ! printf '%s\n' "$out" | grep -q '^exit 0$' \
     && printf '%s\n' "$out" | grep -q 'OpenBLAS kernels Brevi does not model'; then
    echo "ok   $f (refused on every path: no OpenBLAS model for this CPU)"
  elif [ "$out" != "$(cat "$exp")" ]; then
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
# --allow-unlocked-python: on Linux, where the kernel locks CPython, it changes nothing: same output, no warning
# (off Linux the build is refused without it, pybridge.rs lock_refusal; the PC's Windows checks cover that side).
out=$(cd tests/bridge && $B run --sandbox --allow-py brlib --allow-unlocked-python lib_values.py 2>&1; echo "exit $?")
if [ "$(printf '%s\n' "$out" | grep -v '^{"path"')" = "$want" ]; then echo "ok   sandbox bridge --allow-unlocked-python on Linux"; else echo "FAIL sandbox bridge --allow-unlocked-python on Linux"; printf '%s\n' "$out"; fail=1; fi
# build (and check, emit, test, fix) take it too: the unknown-option check must know it (run --verify builds with it).
t=$(mktemp -d)
out=$(cd tests/bridge && $B build --sandbox --allow-py brlib --allow-unlocked-python lib_values.py -o "$t/lv" 2>&1 && "$t/lv" 2>&1; echo "exit $?")
if [ "$(printf '%s\n' "$out" | grep -v '^{"path"')" = "$want" ]; then echo "ok   sandbox bridge build --allow-unlocked-python"; else echo "FAIL sandbox bridge build --allow-unlocked-python"; printf '%s\n' "$out"; fail=1; fi
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
  # The dynamic build carries its CPython too.
  # Brevi's own sqlite3 would load the system's libsqlite3, which the binary does not carry: CPython runs it.
  BREVI_DYN=1 $BREVI build "$f" --standalone --explain -o "$t/dyn" 2>&1 | grep -q '"py":\[[^]]*"sqlite3"' || out="$out
dynamic build: sqlite3 not handed to the bundled CPython"
  dyn=$(env -i HOME="$t" PATH=/nonexistent BREVI_PY_CACHE="$t/cache" "$t/dyn" 2>&1)
  [ "$dyn" = "$(cat "$exp")" ] || out="$out
dynamic build: $dyn"
  # Bytes appended after the build (as code signing does) hide the bundle: refused, even with python3 on PATH,
  # never run on the installed Python instead.
  cp "$t/prog" "$t/signed"; printf 'appended after the build' >> "$t/signed"
  cut=$(BREVI_PY_CACHE="$t/cache2" "$t/signed" 2>&1; echo "exit $?")
  printf '%s\n' "$cut" | grep -q 'built with --standalone, but its bundled Python is missing' && ! printf '%s\n' "$cut" | grep -qx 'exit 0' || out="$out
appended to: $cut"
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
  for c in "a b|a b" "a --native b|a --native b" "a -- --x|a -- --x" "-- --x a|--x a" "--fast a b|a b" "a b -- c|a b -- c" "-- --fast --sandbox a|--fast --sandbox a"; do
    out=$(BREVI_DYN=$d $BREVI run tests/cli/argv.py --native ${c%%|*} 2>&1); want=$("${BREVI_PYTHON:-python3}" tests/cli/argv.py ${c#*|})
    if [ "$out" = "$want" ]; then echo "ok   cli args ($d: ${c%%|*})"; else echo "FAIL cli args ($d: ${c%%|*})"; echo "want $want"; echo "got  $out"; fail=1; fi
  done
  out=$(BREVI_DYN=$d $BREVI run tests/cli/argv.py --native --sandbx 2>&1; echo "exit $?")
  if printf '%s\n' "$out" | grep -q 'no option --sandbx' && printf '%s\n' "$out" | grep -qx 'exit 2'; then echo "ok   cli unknown option refused ($d)"; else echo "FAIL cli unknown option refused ($d)"; printf '%s\n' "$out"; fail=1; fi
  # Every command refuses an option it does not have, before or after FILE (a misspelt --sandbox built an
  # unsandboxed binary and exited 0), and no output is written.
  t=$(mktemp -d)
  for c in "build tests/cli/argv.py --sandbx -o $t/x|brevi build has no option --sandbx (did you mean --sandbox?)" \
           "build --sandbx tests/cli/argv.py -o $t/x|brevi build has no option --sandbx" \
           "check tests/cli/argv.py --nope|brevi check has no option --nope" \
           "emit tests/cli/argv.py --relase|brevi emit has no option --relase (did you mean --release?)" \
           "fix tests/cli/argv.py -q|brevi fix has no option -q" \
           "test tests/cli/argv.py --nocapture|put it after -- (brevi test FILE -- --nocapture)" \
           "run --sandbx tests/cli/argv.py|brevi run has no option --sandbx" \
           "build tests/cli/argv.py -o=$t/x|brevi build has no option -o=" \
           "build tests/cli/argv.py --verify -o $t/x|brevi build has no option --verify" \
           "run tests/cli/argv.py --timeout 5|--timeout is an option of brevi run --verify" \
           "biuld tests/cli/argv.py|brevi has no command biuld"; do
    out=$(BREVI_DYN=$d $BREVI ${c%%|*} --native 2>&1; echo "exit $?")
    if printf '%s\n' "$out" | grep -qF -- "${c#*|}" && printf '%s\n' "$out" | grep -qx 'exit 2' && [ ! -e "$t/x" ]; then echo "ok   cli unknown option refused ($d: ${c%%|*})"; else echo "FAIL cli unknown option refused ($d: ${c%%|*})"; printf '%s\n' "$out"; fail=1; fi
  done
  out=$(BREVI_DYN=$d $BREVI build tests/cli/argv.py --native --allow=read --limit-free 2>&1; echo "exit $?")
  printf '%s\n' "$out" | grep -qF 'no option --limit-free' || { echo "FAIL cli option after an = form ($d)"; printf '%s\n' "$out"; fail=1; }
  if BREVI_DYN=$d $BREVI build tests/cli/argv.py --native --allow=read -o "$t/x" >/dev/null 2>&1 && [ -x "$t/x" ]; then echo "ok   cli --allow=VALUE taken ($d)"; else echo "FAIL cli --allow=VALUE taken ($d)"; fail=1; fi
  rm -rf "$t"
  for f in fstring_spec bom; do
    out=$(BREVI_DYN=$d $BREVI run tests/cli/$f.py --native 2>&1); want=$("${BREVI_PYTHON:-python3}" tests/cli/$f.py 2>&1)
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
# BREVI_DYN=0 keeps the wrong-answer guard: a file it sends to the dynamic build is refused, never built typed.
out=$(BREVI_DYN=0 $BREVI run tests/cli/guard_dyn0.py --native 2>&1; echo "exit $?")
if printf '%s\n' "$out" | grep -q "CPython's hash order, and this file cannot be built over dynamic values (BREVI_DYN=0 turns the dynamic build off)" && printf '%s\n' "$out" | grep -qx 'exit 1'; then echo "ok   cli BREVI_DYN=0 keeps the guard"; else echo "FAIL cli BREVI_DYN=0 keeps the guard"; printf '%s\n' "$out"; fail=1; fi
out=$($BREVI run tests/cli/fstring_brace_field.py --native 2>/dev/null)
if [ "$out" = "$("${BREVI_PYTHON:-python3}" tests/cli/fstring_brace_field.py)" ]; then echo "ok   cli f-string field"; else echo "FAIL cli f-string field"; printf '%s\n' "$out"; fail=1; fi
# A reader that stops early ends brevi report quietly (it panicked on the closed pipe).
t=$(mktemp -d); i=0
while [ $i -lt 400 ]; do mkdir -p "$t/differs/r$i"; printf '{"file":"x.py","on":"stdout"}\n' > "$t/differs/r$i/verdict.json"; i=$((i+1)); done
BREVI_CACHE="$t" $BREVI report 2>"$t/err" | head -1 >/dev/null
if [ ! -s "$t/err" ]; then echo "ok   cli report into a closed pipe"; else echo "FAIL cli report into a closed pipe"; cat "$t/err"; fail=1; fi
rm -rf "$t"
#== cargo_home
# Crate builds run cargo, and cargo runs what its home and any .cargo/config.toml above its folder name (a rustc
# wrapper here). So a cargo home other users can change is not used (a private one stands in), and cargo never
# runs in the caller's folder, where another user of a shared /workspace or /tmp could leave a .cargo/config.toml.
# The control run shows the planted wrapper would run otherwise.
tk=$(mktemp -d); chmod 755 "$tk"; cp tests/run/crate_use.bv "$tk/p.bv"; tkb=$(realpath "$BREVI")
printf '#!/bin/sh\ntouch "%s/planted"\nexec "$@"\n' "$tk" > "$tk/wrap"; chmod 755 "$tk/wrap"
for d in own shared cwd/.cargo; do mkdir -p "$tk/$d"; printf '[build]\nrustc-wrapper = "%s/wrap"\n' "$tk" > "$tk/$d/config.toml"; done
chmod 777 "$tk/shared"; chmod 1777 "$tk/cwd"; mkdir "$tk/cwd/sub"
tkr() { rm -f "$tk/planted"; h=$1; shift; (cd "$tk/${1:-.}" && env CARGO_HOME="$tk/$h" BREVI_CACHE="$tk/cache-$h${1:+-cwd}" "$tkb" run "$tk/p.bv" 2>&1 | sed 's/^brevi: not using the cargo home .*: .* can be changed by other users.*/refused shared home/'); [ -e "$tk/planted" ] && echo planted; }
tkown=$(tkr own)
tkout="$tkown / $(tkr shared) / $(mkdir -p "$tk/home2"; tkr home2 cwd/sub)"
want='ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
planted / refused shared home
ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad / ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad'
# The test fetches a crate. With no route to crates.io (an offline sandbox) the control run itself cannot build:
# that is SKIP, never ok, and only when the control run's own error is a network/offline one.
if [ "$tkout" = "$want" ]; then echo "ok   cargo home trust: a shared home or a planted .cargo/config.toml is not used"
elif ! printf '%s\n' "$tkown" | grep -q '^ba7816bf' && printf '%s\n' "$tkown" | grep -Eqi "offline mode|could not resolve host|couldn't resolve|failed to download|spurious network error|failed to query replaced source|network failure|failed to get .* as a dependency"; then
  echo "SKIP cargo home trust: crates.io cannot be reached here"; printf '%s\n' "$tkown" | tail -3
else echo "FAIL cargo home trust"; printf '%s\n' "$tkout"; fail=1; fi
chmod -R u+rwX "$tk"; rm -rf "$tk"
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
# The cache is trusted: a replay runs the binary it holds. So no folder grant may reach it (a sandboxed program
# granted write there could plant a binary a later run executes with no hold), and a cache other users can change
# is not used at all: a private one stands in. The control run shows a planted binary would run otherwise.
tc=$(mktemp -d); printf 'print(6 * 7)\n' > "$tc/main.py"; mkdir "$tc/out"
cc() { env BREVI_CACHE="$tc/cache" $BREVI "$@" "$tc/main.py" 2>&1; echo "exit $?"; }
tcout=$(cc run >/dev/null
  for b in "$tc"/cache/runs/*/bin; do printf '#!/bin/sh\necho planted\n' > "$b"; done
  echo "-- control"; cc run
  echo "-- grant holding the cache"; cc run --sandbox --allow-write "$tc" | sed -n 's/.*the folder holds Brevi.s build cache.*/holds the cache/p; /^exit/p'
  echo "-- grant inside the cache"; cc run --sandbox --allow-write "$tc/cache/runs" | sed -n 's/.*the folder is inside Brevi.s build cache.*/inside the cache/p; /^exit/p'
  echo "-- grant beside it"; cc run --sandbox --allow-write "$tc/out" | grep -v planted
  echo "-- shared"; chmod 777 "$tc/cache/runs"; cc run | sed 's/ [^ ]*brevi-[0-9]*\/cache / PRIVATE /; s/build cache [^:]*:/build cache CACHE:/; s/: [^ ]*\/runs can/: RUNS can/')
tcwant='-- control
planted
exit 0
-- grant holding the cache
holds the cache
exit 2
-- grant inside the cache
inside the cache
exit 2
-- grant beside it
42
exit 0
-- shared
brevi: not using the build cache CACHE: RUNS can be changed by other users (mode 777); using PRIVATE instead (set BREVI_CACHE to a folder only you can write)
42
exit 0'
if [ "$tcout" = "$tcwant" ]; then echo "ok   cache trust: no grant reaches it, a shared one is not used"
else echo "FAIL cache trust"; printf '%s\n' "$tcwant" > "$tc/want"; printf '%s\n' "$tcout" | diff "$tc/want" -; fail=1; fi
chmod -R u+rwX "$tc"; rm -rf "$tc"
# BREVI_CACHE_SEED: a read-only folder of runtime libraries built ahead (the sandbox image's, made as root). A
# fresh cache links the seed's library instead of building its own, unless others could change the seed.
ts=$(mktemp -d); chmod 755 "$ts"; printf 'print(sum([1, 2, 3]))\n' > "$ts/main.py"
tsr() { env BREVI_CACHE="$ts/$1" BREVI_CACHE_SEED="$ts/seed" BREVI_TIERED=0 $BREVI run --native "$ts/main.py" 2>&1; }
env BREVI_CACHE="$ts/seed" BREVI_TIERED=0 $BREVI run --native "$ts/main.py" >/dev/null 2>&1; chmod -R go-w "$ts/seed"
tsout="$(tsr own) $(ls "$ts/own" | grep -c '^rt$')"
chmod o+w "$ts/seed/rt"
tsout="$tsout / $(tsr own2) $(ls "$ts/own2" | grep -c '^rt$')"
if [ "$tsout" = "6 0 / 6 1" ]; then echo "ok   cache seed: used when only its owner can change it"
else echo "FAIL cache seed: $tsout (want 6 0 / 6 1)"; fail=1; fi
rm -rf "$ts"
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
o=$(BREVI_PYTHON=tests/verify/py314.sh $BREVI run tests/fallback/py_exec.py 2>&1
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
# With the build folder (TMPDIR) on another file system or mount than the cache, cached programs are copied,
# not hard linked, and a copy written by a brevi process with other threads (the rustc probe at start-up) could
# still be open in a child that thread was starting when the program ran: "Text file busy" in about 1 run in 60
# at 8 at once. Copies are made by a single-threaded helper (copy_exe), so none of these runs may fail.
t5=$(mktemp -d)
t5t=/dev/shm/brevi-check-$$
t5k="${BREVI_CACHE:-$HOME/.cache/brevi}"
mkdir -p "$t5k"
if mkdir -p "$t5t" 2>/dev/null && [ "$(stat -c %d "$t5t")" != "$(stat -c %d "$t5k")" ]; then
  printf 'import sys\nprint(sys.argv[1:], %s)\n' "$$" > "$t5/p.py"
  BREVI_DYN=1 BREVI_TIERED=0 TMPDIR="$t5t" $BREVI run "$t5/p.py" --native w > /dev/null 2>&1
  for j in 1 2 3 4 5 6 7 8; do
    ( mkdir -p "$t5t/t$j"; i=0; while [ $i -lt 60 ]; do
        BREVI_DYN=1 BREVI_TIERED=0 TMPDIR="$t5t/t$j" $BREVI run "$t5/p.py" --native $j $i 2>&1; i=$((i + 1)); done > "$t5/out$j" ) &
  done
  wait
  t5bad=0; j=1
  while [ $j -le 8 ]; do
    i=0; while [ $i -lt 60 ]; do echo "['$j', '$i'] $$"; i=$((i + 1)); done | cmp -s - "$t5/out$j" || t5bad=$((t5bad + 1)); j=$((j + 1))
  done
  if [ $t5bad = 0 ]; then echo "ok   cached programs copied across file systems run (480 runs, 8 at once)"
  else echo "FAIL cached programs copied across file systems run: $t5bad of 8 loops went wrong"; grep -hv "^\[" "$t5"/out* | sort | uniq -c | head; fail=1; fi
else
  echo "ok   cached programs copied across file systems run (skipped: no /dev/shm on another file system than the cache)"
fi
rm -rf "$t5" "$t5t"
exit $fail

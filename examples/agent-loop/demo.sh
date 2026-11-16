#!/usr/bin/env bash
# The agent loop, end to end, with real output from the machine it runs on:
#   1. an agent writes plain Python (draft.py, with a typo)
#   2. Brevi compiles it and answers with one JSON line naming the line and a fix
#   3. the agent fixes that line (main.py), and `brevi run --verify` runs it natively and checks it against CPython
#   4. a program Brevi cannot check safely is refused visibly, never run unchecked as if verified
#   5. the same program through the MCP server, as a harness would call it
#   6. latency against CPython on a bigger input, timed here
# Needs: brevi on PATH, and CPython 3.13 (python3, or BREVI_PYTHON) for the CPython comparison.
# Each step checks its own result, so CI uses this script as the install smoke test: it exits non-zero
# when any step answers differently from what is written here.
set -euo pipefail
cd "$(dirname "$0")"
# The CPython used for MCP and timing: the 3.13 Brevi compares against, found the way Brevi finds it
PY=${BREVI_PYTHON:-$(command -v python3.13 || uv python find 3.13 2>/dev/null || command -v python3 || command -v python)}
"$PY" -c 'import sys; assert sys.version_info[:2] == (3, 13), "demo needs CPython 3.13: uv python install 3.13"' 
work=$(mktemp -d)
# Git Bash on Windows: hand Windows paths to the Windows python
win() { if command -v cygpath >/dev/null; then cygpath -w "$1"; else printf '%s' "$1"; fi; }
trap 'rm -rf "$work"' EXIT
step() { printf '\n\033[1m== %s\033[0m\n' "$*"; }
fail() { echo "demo: FAIL: $*" >&2; exit 1; }
show() { printf '$ %s\n' "$*"; }

brevi --version

step "1. The agent's first draft has a typo on line 20"
show "brevi check draft.py"
if out=$(brevi check draft.py 2>&1); then fail "check accepted draft.py"; fi
echo "$out"
case "$out" in *'"line":20'*'did you mean total?'*) ;; *) fail "unexpected check output";; esac

step "2. The agent fixes line 20 and runs with --verify"
show "echo 1000 | brevi run --verify main.py"
echo 1000 | brevi run --verify main.py >"$work/out" 2>"$work/err" || fail "run --verify failed: $(cat "$work/err")"
cat "$work/out" "$work/err"
grep -q '"verify":"identical"' "$work/err" || fail "no identical certificate"
expected=$(printf '168 primes below 1000\nsum 76127')
[ "$(tr -d '\r' <"$work/out")" = "$expected" ] || fail "wrong output"

step "3. A program that reads the clock cannot be checked by running it twice: refused, and said so"
printf 'import time\nprint("clock read:", time.time() > 0)\n' >"$work/clock.py"
show "brevi run --verify clock.py"
brevi run --verify "$work/clock.py" </dev/null 2>&1 | tee "$work/clk" || true
grep -q '"verify":"refused"' "$work/clk" || fail "clock program was not refused"

step "4. The same fix through the MCP server (sandboxed by default, verify on by default)"
show "python3 mcp_run.py main.py 1000"
"$PY" mcp_run.py main.py 1000 | tee "$work/mcp"
grep -q '"verify":"identical"' "$work/mcp" || fail "MCP run was not verified"

step "5. Latency against CPython, on N = 20,000,000 (timed on this machine)"
brevi build main.py -o "$work/primes" >/dev/null
"$PY" - "$(win "$work/primes")" "$(win "$PY")" <<'PYEOF'
import subprocess, sys, time
binary, py = sys.argv[1], sys.argv[2]
def best(cmd):
    t = []
    for _ in range(3):
        s = time.perf_counter()
        out = subprocess.run(cmd, input=b"20000000", capture_output=True, check=True).stdout
        t.append(time.perf_counter() - s)
    return min(t), out
nat, a = best([binary])
cpy, b = best([py, "main.py"])
if a != b:
    sys.exit("demo: FAIL: outputs differ on the big input")
print(a.decode().strip())
print(f"native {nat*1000:.0f} ms, CPython {cpy*1000:.0f} ms: {cpy/nat:.1f}x (best of 3, same output)")
PYEOF

echo
echo "demo: all steps passed"

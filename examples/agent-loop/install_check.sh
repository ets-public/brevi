#!/usr/bin/env bash
# The quickstart's install, step by step, as install-test.yml runs it on a clean machine:
#   1. `uvx --python 3.13 brevi run --verify main.py`: fetches brevi, CPython 3.13 and (with no rustc on the PATH)
#      the pinned Rust, builds main.py, runs it and checks it against CPython
#   2. `uv tool install --python 3.13 brevi`: keeps `brevi` on the PATH for the rest of the quickstart (and demo.sh)
# Usage: install_check.sh PATH-TO-UV. BREVI_VERSION (a release tag or version) pins brevi to that release and waits
# for it to reach PyPI first; empty means the latest on PyPI, exactly as the docs say.
set -euo pipefail
uv=$1
case "$uv" in *.exe) uvx="${uv%uv.exe}uvx.exe";; *) uvx="${uv}x";; esac
here=$(cd "$(dirname "$0")" && pwd)
fail() { echo "install_check: FAIL: $*" >&2; exit 1; }

v=${BREVI_VERSION:-}
v=${v#v}
run_spec=brevi tool_spec=brevi
if [ -n "$v" ]; then
  run_spec="brevi@$v" tool_spec="brevi==$v"
  # A release publishes to PyPI a few minutes after the GitHub release: wait up to 30 minutes for it.
  for _ in $(seq 60); do
    curl -fsS -o /dev/null "https://pypi.org/pypi/brevi/$v/json" && break
    echo "waiting for brevi $v on PyPI"; sleep 30
  done
  curl -fsS -o /dev/null "https://pypi.org/pypi/brevi/$v/json" || fail "brevi $v is not on PyPI"
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cp "$here/main.py" "$work/main.py"
cd "$work"

echo "\$ uvx --python 3.13 $run_spec run --verify main.py"
echo 1000 | "$uvx" --python 3.13 "$run_spec" run --verify main.py >out 2>err || fail "uvx run --verify failed: $(cat err)"
cat out err
grep -q '"verify":"identical"' err || fail "no identical certificate"
[ "$(tr -d '\r' <out)" = "$(printf '168 primes below 1000\nsum 76127')" ] || fail "wrong output"

echo "\$ uv tool install --python 3.13 $tool_spec"
"$uv" tool install --python 3.13 "$tool_spec"
bin=$("$uv" tool dir --bin)
command -v cygpath >/dev/null && bin=$(cygpath -u "$bin")
got=$("$bin/brevi" --version)
echo "$got"
[ -z "$v" ] || case "$got" in *"$v"*) ;; *) fail "uv tool install gave $got, not $v";; esac
echo "install_check: passed"

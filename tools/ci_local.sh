#!/bin/sh
# The pull-request checks from .github/workflows/toolchains.yml, run on this machine.
# GitHub Actions minutes are limited, so run this before merging. Run from the repo root.
#   sh tools/ci_local.sh          the checks a PR needs: the old-solutions rebuild, the grammar check and the
#                                 tests of Brevi's library copies (mcp, openai, polars, ...) run only when what this
#                                 branch changes (against origin/main) can affect them. GitHub CI runs them all.
#   sh tools/ci_local.sh --full   everything, as the daily run and a release do
#   sh tools/ci_local.sh --quick  skip the old-solutions rebuild (only for changes outside the compiler)
# The language tests run in parallel (tests/check_parallel.py, one job per CPU; JOBS=N to change that).
set -e
step() { printf '\n== %s  (%s)\n' "$1" "$(date +%H:%M:%S)"; }
# Room for every test program and old solution, so a second run finds them all cached (the default 1 GB evicts mid-run).
export BREVI_CACHE_MB="${BREVI_CACHE_MB:-6144}"
mode=${1:-pr}
fast=
compat=1; kinds=brevi,python,rust; grammar=1; stdlib=1
if [ "$mode" = --quick ]; then compat=0; fi
if [ "$mode" = pr ]; then
  # The same rules as toolchains.yml's "Does this PR change the compiler or the baseline?" step.
  base=$(git merge-base HEAD origin/main 2>/dev/null || true)
  if [ -n "$base" ]; then
    fast="--changed-since $base"
    changed=$( { git diff --name-only "$base"; git ls-files --others --exclude-standard; } )
    echo "$changed" | grep -qE '^(src/|Cargo\.(toml|lock)$|compat/baseline\.txt$|tools/compat\.py$|bench/(solutions|tasks|ref)/)' || compat=0
    echo "$changed" | grep -qE '^(compat/baseline\.txt$|tools/compat\.py$|bench/(solutions|tasks)/)' || kinds=brevi
    # The stdlib slice (~6 min on 4 cores): GitHub runs it only after the merge (stdlib.yml), so check it here
    # whenever the compiler or the slice's lists change. Merge origin/main first to see what
    # this branch does together with the PRs merged since it started.
    echo "$changed" | grep -qE '^(src/|Cargo\.(toml|lock)$|tools/conformance\.py$|compat/conformance_(stdlib_|by_design))' || stdlib=0
    echo "$changed" | grep -qE '^(grammar/|tests/run/|bench/solutions/|compat/baseline\.txt$|tools/(grammar_check|lark_to_gbnf)\.py$|tools/requirements-ci\.txt$)' || grammar=0
  fi
fi
step "Toolchain"; rustc -V; python3 -V
step "Build"; cargo build --release -q
step "Unit tests"; cargo test --release -q
step "Language tests"; BREVI=./target/release/brevi python3 tests/check_parallel.py --quiet $fast
step "MCP server"; python3 tests/mcp_test.py && python3 tests/mcp_http_test.py
step "Model box"; python3 tests/modelbox_test.py
step "Python SDK and brevi-python"; BREVI=./target/release/brevi python3 tests/sdk_test.py
BREVI=./target/release/brevi node tests/sdk_node_test.mjs
step "Guide budget"; python3 tools/guide_budget.py
step "Package versions"; python3 tools/version_sync.py
step "Changelog fragments"; python3 tools/changelog.py --check
if [ $grammar = 1 ]; then
  step "Grammar"; python3 -c "import lark" 2>/dev/null || python3 -m pip install --quiet -r tools/requirements-ci.txt
  python3 tools/lark_to_gbnf.py --check; python3 tools/grammar_check.py tests/run/*.bv
fi
step "GUI runtime matches tkinter stand-in"; sh tools/gui_shim_check.sh
step "GUI window front end compiles (eframe)"; cargo check -q --manifest-path runtime/brevi_tk/Cargo.toml --features window
step "Bench scripts compile"; python3 -m py_compile bench/*.py bench/ref/*.py tools/*.py
step "Bench DeepSeek provider"; python3 -W ignore bench/test_deepseek.py >/dev/null 2>&1 || python3 -W ignore bench/test_deepseek.py
step "Agent platform templates"; python3 templates/test.py >/dev/null
if [ $compat = 1 ]; then
  step "Every solution that ever passed still passes ($kinds)"; python3 tools/compat.py check --kinds $kinds
fi
# CPython's own tests, when the compiler could have changed (as in toolchains.yml)
if [ $compat = 1 ] || [ "$mode" = --full ]; then
  step "CPython conformance ratchet (CPython 3.13.13 via uv; ~10 min)"
  if command -v uv >/dev/null 2>&1 && uv python install 3.13.13 --quiet 2>/dev/null; then
    python3 tools/conformance.py --check --python "$(uv python find 3.13.13)" --out /tmp/brevi-conformance
    if [ $stdlib = 1 ]; then
      step "CPython conformance ratchet, stdlib slice (~6 min)"
      python3 tools/conformance.py --slice stdlib --check --python "$(uv python find 3.13.13)" --out /tmp/brevi-conformance-stdlib
    else
      echo "(stdlib slice skipped: nothing here can change it; main runs it after the merge)"
    fi
  else
    echo "skipped: uv not installed (https://docs.astral.sh/uv/), so no CPython 3.13.13 to compare with"
  fi
else
  echo "(CPython conformance ratchet skipped: nothing here can change it)"
fi
[ $compat = 1 ] || echo "(old-solutions rebuild skipped: nothing here can change it)"
[ $grammar = 1 ] || echo "(grammar check skipped: nothing here can change it)"
printf '\nAll checks passed.  (%s)\n' "$(date +%H:%M:%S)"

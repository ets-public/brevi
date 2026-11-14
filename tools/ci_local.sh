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
compat=1; kinds=brevi,python,rust; grammar=1; stdlib=1; conf=1
if [ "$mode" = --quick ]; then compat=0; conf=0; fi
if [ "$mode" = pr ]; then
  # The same rules as toolchains.yml's "Does this PR change the compiler or the baseline?" step.
  base=$(git merge-base HEAD origin/main 2>/dev/null || true)
  if [ -n "$base" ]; then
    fast="--changed-since $base"
    changed=$( { git diff --name-only "$base"; git ls-files --others --exclude-standard; } )
    echo "$changed" | grep -qE '^(src/|Cargo\.(toml|lock)$|compat/baseline\.txt$|tools/compat\.py$|bench/(solutions|tasks|ref)/)' || compat=0
    echo "$changed" | grep -qE '^(compat/baseline\.txt$|tools/compat\.py$|bench/(solutions|tasks)/)' || kinds=brevi
    # The stdlib slice (~6 min on 4 cores): GitHub runs it only after the merge (conformance-main.yml), so check it here
    # whenever the compiler or the slice's lists change. Merge origin/main first to see what
    # this branch does together with the PRs merged since it started.
    echo "$changed" | grep -qE '^(src/|Cargo\.(toml|lock)$|tools/conformance\.py$|compat/conformance_(stdlib_|by_design))' || stdlib=0
    # A conformance list change gets no GitHub run (toolchains.yml skips it), so both slices run here instead.
    echo "$changed" | grep -qE '^(tools/conformance(\.py|_candidates\.json)$|compat/conformance_)' && stdlib=1 || conf=0
    echo "$changed" | grep -qE '^(grammar/|tests/run/|bench/solutions/|compat/baseline\.txt$|tools/(grammar_check|lark_to_gbnf)\.py$|tools/requirements-ci\.txt$)' || grammar=0
  fi
fi
# The GitHub-only "versions" job checks, for the PRs GitHub CI skips (docs, site, conformance lists).
step "Workflows, credentials, built files, --limit claims"
python3 tools/workflow_guard.py && python3 tools/secret_scan.py && python3 tools/no_binaries.py && python3 tools/limits_check.py
# Brevi answers as CPython 3.13 does and runs no other CPython (src/cpython.rs), so the checks run with python3 =
# CPython 3.13.13 (uv's build, the one conformance compares with), as GitHub CI runs them with 3.13. The machine's
# own python3 (another version, say 3.15) stays out of the run, except for the model box check below.
sys_py=$(command -v python3)
if command -v uv >/dev/null 2>&1 && uv python install 3.13.13 --quiet 2>/dev/null && py313=$(uv python find 3.13.13 2>/dev/null); then
  export PATH="$(dirname "$py313"):$PATH"
  unset BREVI_PYTHON
else
  case "$(python3 -V 2>&1)" in "Python 3.13."*) ;; *) echo "ci_local: needs CPython 3.13 (install uv, https://docs.astral.sh/uv/, which fetches 3.13.13), and python3 is $(python3 -V 2>&1)" >&2; exit 1;; esac
fi
step "Toolchain"; rustc -V; python3 -V
# The same Rust as GitHub CI (.github/rust-version), or results differ: a newer rustc's Unicode tables change what
# re.IGNORECASE accepts. Fix with: rustup toolchain install X && rustup default X
pin=$(cat .github/rust-version)
case "$(rustc -V)" in "rustc $pin "*) ;; *) echo "ci_local: rustc must be $pin, as in GitHub CI (rustup default $pin)" >&2; exit 1;; esac
step "Build"; cargo build --release -q
# The MCP checks compare Brevi's servers with CPython running the real mcp, so they need the same mcp stack as GitHub
# (tests/requirements-native.txt without duckdb and onnxruntime): with another pydantic, CPython's own session ends
# differently and `brevi verify` reports "differs" on correct code. A venv over this Python's own packages holds the
# pins, made once per pin set. It goes on PATH only for the MCP checks, as on GitHub: the CPython sandbox hides
# $HOME, so a python3 from there fails every sandboxed check (tests/sandbox, limits, bridge_sandbox, modelbox).
if command -v uv >/dev/null 2>&1; then
  pins=$(grep -vE '^(#|duckdb==|onnxruntime==)' tests/requirements-native.txt | grep .)
  venv="${BREVI_CACHE:-$HOME/.cache/brevi}/ci-venv-$( { python3 -VV; echo "$pins"; } | sha256sum | cut -c1-12)"
  if [ ! -x "$venv/bin/python3" ]; then
    step "Pinned mcp stack for the MCP checks (once per pin set)"
    uv venv -q --python "$(command -v python3)" --system-site-packages "$venv.tmp" &&
      echo "$pins" | uv pip install -q --python "$venv.tmp/bin/python" -r /dev/stdin && mv "$venv.tmp" "$venv" ||
      { rm -rf "$venv.tmp"; echo "could not make $venv"; exit 1; }
  fi
  mcp_path="$venv/bin:$PATH"
else
  mcp_path="$PATH"
  echo "warning: uv not installed (https://docs.astral.sh/uv/): the MCP checks compare against this Python's own mcp and pydantic, not the pinned ones"
fi
step "Unit tests"; cargo test --release -q
step "Language tests"; BREVI=./target/release/brevi CHECK_SKIP=mcp_sdk,mcp_http python3 tests/check_parallel.py --quiet $fast
step "MCP servers against the pinned mcp"; PATH="$mcp_path" BREVI=./target/release/brevi CHECK_ONLY=mcp_sdk,mcp_http python3 tests/check_parallel.py --quiet $fast
step "MCP server"; python3 tests/mcp_test.py && PATH="$mcp_path" python3 tests/mcp_http_test.py
# (with the machine's own python3: the box hides $HOME, where uv keeps its CPython)
step "Model box"; "$sys_py" tests/modelbox_test.py
step "Python SDK and brevi-python"; BREVI=./target/release/brevi python3 tests/sdk_test.py
BREVI=./target/release/brevi node tests/sdk_node_test.mjs
step "Guide budget"; python3 tools/guide_budget.py
step "Code map summaries and agent hooks"; python3 tools/code_map.py --check && python3 tests/agent_hooks_test.py
step "Package versions"; python3 tools/version_sync.py
step "Changelog fragments"; python3 tools/changelog.py --check
if [ "$mode" = pr ] && [ -n "$base" ]; then step "Conformance floors left to main"; python3 tools/floor_guard.py "$base"; fi
if [ "$mode" = pr ] && [ -n "$base" ]; then step "CHANGELOG.md and published scores left alone"; PR_TITLE="$(git log -1 --format=%s)" python3 tools/pr_guard.py "$base"; fi
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
if [ $compat = 1 ] || [ $conf = 1 ] || [ "$mode" = --full ]; then
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

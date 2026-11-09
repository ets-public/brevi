#!/bin/sh
# The pull-request checks from .github/workflows/toolchains.yml, run on this machine.
# GitHub Actions minutes are limited, so run this before merging. Run from the repo root.
#   sh tools/ci_local.sh          everything, including the ~6 minute rebuild of old solutions
#   sh tools/ci_local.sh --quick  skip the old-solutions rebuild (only for changes outside the compiler)
set -e
step() { printf '\n== %s\n' "$1"; }
step "Toolchain"; rustc -V; python3 -V
step "Build"; cargo build --release -q
step "Unit tests"; cargo test --release -q
step "Language tests"; BREVI=./target/release/brevi sh tests/check.sh | grep -v '^ok' || true
BREVI=./target/release/brevi sh tests/check.sh >/dev/null
step "MCP server"; python3 tests/mcp_test.py && python3 tests/mcp_http_test.py
step "Python SDK and brevi-python"; BREVI=./target/release/brevi python3 tests/sdk_test.py
BREVI=./target/release/brevi node tests/sdk_node_test.mjs
step "Guide budget"; python3 tools/guide_budget.py
step "Package versions"; python3 tools/version_sync.py
step "Grammar"; python3 -c "import lark" 2>/dev/null || python3 -m pip install --quiet -r tools/requirements-ci.txt
python3 tools/lark_to_gbnf.py --check; python3 tools/grammar_check.py tests/run/*.bv
step "GUI runtime matches tkinter stand-in"; sh tools/gui_shim_check.sh
step "GUI window front end compiles (eframe)"; cargo check -q --manifest-path runtime/brevi_tk/Cargo.toml --features window
step "Bench scripts compile"; python3 -m py_compile bench/*.py bench/ref/*.py tools/*.py
step "Bench DeepSeek provider"; python3 -W ignore bench/test_deepseek.py >/dev/null 2>&1 || python3 -W ignore bench/test_deepseek.py
step "Agent platform templates"; python3 templates/test.py >/dev/null
if [ "$1" != --quick ]; then
  step "Every solution that ever passed still passes"; python3 tools/compat.py check
fi
printf '\nAll checks passed.\n'

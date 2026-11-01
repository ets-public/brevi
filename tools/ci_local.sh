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
step "Guide budget"; python3 tools/guide_budget.py
step "Package versions"; python3 tools/version_sync.py
step "Bench scripts compile"; python3 -m py_compile bench/*.py bench/ref/*.py tools/*.py
if [ "$1" != --quick ]; then
  step "Every solution that ever passed still passes"; python3 tools/compat.py check
fi
printf '\nAll checks passed.\n'

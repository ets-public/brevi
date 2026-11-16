#!/bin/sh
# The heavy half of the security assurance programme (docs/SECURITY_TESTING.md), for Adam's PC (WSL Ubuntu-24.04)
# under the CI lock, never on GitHub (Actions minutes are limited) and never on model-written branches.
#
#   sh tools/security_checks.sh [SECONDS_PER_FUZZ_TARGET]     (default 600; 0 skips fuzzing)
#
# Steps, each reported PASS / FAIL / SKIP in security-checks/summary.txt:
#   1. cargo audit and cargo deny on every Cargo.lock (/, runtime/brevi_tk, fuzz)
#   2. cargo geiger on the compiler, and tools/unsafe_inventory.py (most unsafe code is pasted into programs,
#      which geiger cannot see)
#   3. the compiler's unit tests under AddressSanitizer (nightly, -Zbuild-std)
#   4. every cargo fuzz target for SECONDS each, ASan and debug assertions on, from the kept corpus
# Needs: rustup nightly with rust-src; cargo-audit, cargo-deny, cargo-geiger, cargo-fuzz (cargo install each once).
# A tool that is missing is a SKIP, never a PASS.
set -u
SECS=${1:-600}
cd "$(dirname "$0")/.."
OUT=security-checks
mkdir -p "$OUT"
: > "$OUT/summary.txt"
nice_run() { nice -n 19 ionice -c 3 "$@" 2>/dev/null || nice -n 19 "$@"; }
note() { echo "$1 $2" | tee -a "$OUT/summary.txt"; }
have() { cargo "$1" --version >/dev/null 2>&1; }
TARGET=$(rustc -vV | sed -n 's/^host: //p')

for lock in . runtime/brevi_tk fuzz; do
  name=$(echo "$lock" | tr / _)
  if have audit; then
    if nice_run cargo audit --file "$lock/Cargo.lock" > "$OUT/audit-$name.log" 2>&1; then note PASS "cargo audit $lock"; else note FAIL "cargo audit $lock ($OUT/audit-$name.log)"; fi
  else note SKIP "cargo audit (cargo install cargo-audit)"; fi
  if have deny; then
    if nice_run cargo deny --manifest-path "$lock/Cargo.toml" check --config deny.toml > "$OUT/deny-$name.log" 2>&1; then note PASS "cargo deny $lock"; else note FAIL "cargo deny $lock ($OUT/deny-$name.log)"; fi
  else note SKIP "cargo deny (cargo install cargo-deny)"; fi
done

if have geiger; then
  nice_run cargo geiger --output-format GitHubMarkdown > "$OUT/geiger.md" 2>"$OUT/geiger.log" && note PASS "cargo geiger ($OUT/geiger.md)" || note FAIL "cargo geiger ($OUT/geiger.log)"
else note SKIP "cargo geiger (cargo install cargo-geiger)"; fi
python3 tools/unsafe_inventory.py > "$OUT/unsafe.txt" && note PASS "unsafe inventory: $(tail -1 "$OUT/unsafe.txt" | sed 's/^ *//')"

if rustup component list --toolchain nightly 2>/dev/null | grep -q '^rust-src (installed)'; then
  if RUSTFLAGS="-Zsanitizer=address" RUSTDOCFLAGS="-Zsanitizer=address" ASAN_OPTIONS=detect_leaks=0 \
     nice_run cargo +nightly test -Zbuild-std --target "$TARGET" --release --target-dir target/asan > "$OUT/asan-tests.log" 2>&1; then
    note PASS "unit tests under AddressSanitizer"
  else note FAIL "unit tests under AddressSanitizer ($OUT/asan-tests.log)"; fi
else note SKIP "ASan unit tests (rustup component add rust-src --toolchain nightly)"; fi

if [ "$SECS" -gt 0 ]; then
  if have fuzz || cargo +nightly fuzz --version >/dev/null 2>&1; then
    for t in $(cargo +nightly fuzz list); do
      if nice_run cargo +nightly fuzz run -O -a "$t" -- -max_total_time="$SECS" -rss_limit_mb=2048 -timeout=10 > "$OUT/fuzz-$t.log" 2>&1; then
        note PASS "fuzz $t ${SECS}s: $(grep -o 'cov: [0-9]*' "$OUT/fuzz-$t.log" | tail -1), $(grep -o 'Done [0-9]* runs' "$OUT/fuzz-$t.log" | tail -1)"
      else note FAIL "fuzz $t: crash in fuzz/artifacts/$t ($OUT/fuzz-$t.log)"; fi
    done
  else note SKIP "fuzzing (cargo install cargo-fuzz)"; fi
fi
grep -q '^FAIL' "$OUT/summary.txt" && exit 1
exit 0

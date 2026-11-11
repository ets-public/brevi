# Loop latency: baseline and the daemon spike

Status: measured on v0.26.0 (9 November 2026), v0.27.0 and main after #414 (11 November 2026) (ROADMAP.md, WS2). Machine: Intel Xeon @ 2.10 GHz, 4 cores, rustc 1.97.0.
Fixture and method: `bench/latency.py` (METRICS.md, "Check latency").

## Main (d91ca4b), 11 November 2026

Machine: Intel Xeon @ 2.10 GHz, 4 cores, rustc 1.97.0, CPython 3.13.18 (the same kind of machine as the v0.26.0 baseline). Release build of main at d91ca4b, with the sharing-proof fix (#414) and the faster guard (#462), both shipping in the next release. `bench/latency-results/2026-11-11-main-d91ca4b[-dyn].json` (the run after #414 alone, at 0349bd8, is kept next to it).

| Mode | Typed path p50 / p95 | Dynamic path (`--dyn`) p50 / p95 |
|---|---|---|
| `brevi check --fast` after an edit | 22.0 / 24.4 ms | 24.0 / 31.4 ms |
| `brevi check` after an edit | 94.4 / 107.4 ms | 97.3 / 454.7 ms |
| `brevi check`, nothing changed | 2.3 / 3.3 ms | 2.4 / 5.4 ms |
| `brevi build` after an edit | 905.0 / 987.0 ms | 1603.0 / 1734.9 ms |
| `brevi run` after an edit (opt-level 0 tier) | 347.7 / 389.8 ms | 531.2 / 599.7 ms |
| `brevi run`, nothing changed | 5.6 / 6.4 ms | 27.6 / 33.2 ms |
| `python3 main.py`, for scale | 15.3 / 17.1 ms | 15.8 / 19.8 ms |

The fixture is on the typed path. Compared with v0.26.0 on the same kind of machine, `check --fast` is 6 ms slower (22.0 against 16.1 ms), still just over the 20 ms target; after #414 alone it was 25.0 ms, and #462 (each file parsed once, no second proof of an imported module) took 3 ms off. The full check (94.4 ms) is back at v0.26.0's 93.6 ms, and a build is 35 ms slower. Tiered builds bring `brevi run` after an edit down to 348 ms, under the 500 ms target.

## v0.27.0, 11 November 2026

Machine: Intel Xeon @ 2.30 GHz, 4 cores, rustc 1.97.0, CPython 3.13.18 (a different machine from the v0.26.0 baseline, so compare loosely). Release binary, `bench/latency-results/2026-11-11-v0.27.0[-dyn].json`.

| Mode | Fixture as written p50 / p95 | Dynamic path (`--dyn`) p50 / p95 |
|---|---|---|
| `brevi check --fast` after an edit | 160.3 / 174.4 ms | 149.9 / 181.0 ms |
| `brevi check` after an edit | 159.8 / 209.3 ms | 156.0 / 164.8 ms |
| `brevi check`, nothing changed | 2.3 / 6.1 ms | 2.0 / 2.8 ms |
| `brevi build` after an edit | 1180.8 / 1253.4 ms | 1182.8 / 1268.8 ms |
| `brevi run` after an edit (opt-level 0 tier) | 427.3 / 469.3 ms | 435.8 / 479.6 ms |
| `brevi run`, nothing changed | 5.4 / 6.1 ms | 20.6 / 23.8 ms |
| `python3 main.py`, for scale | 11.4 / 14.0 ms | 11.9 / 14.3 ms |

**At v0.27.0 the fixture did not take the typed path** (fixed on main, above). The sharing proof added in v0.27.0 (`share_guard.rs`) sends it to the dynamic build: it cannot follow `top_words(counts, 3)` into the sibling module `textutil.py`, so it assumes `counts` is changed. That is why both columns match, and why `check --fast` rose from 16 ms to 160 ms. It was a false refusal, fixed in #414. Tiered builds work as planned: `brevi run` after an edit is 427 ms, against 1181 ms for `brevi build`.

## Baseline, v0.26.0

| Mode | Typed path p50 / p95 | Dynamic path (`--dyn`) p50 / p95 |
|---|---:|---:|
| `brevi check --fast` after an edit | 16.1 / 18.2 ms | 17.6 / 21.5 ms |
| `brevi check` after an edit | 93.6 / 113.6 ms | 100.8 / 401.9 ms |
| `brevi check`, nothing changed | 2.5 / 6.2 ms | 2.6 / 6.3 ms |
| `brevi build` after an edit | 869.6 / 958.9 ms | 1508.3 / 1645.0 ms |
| `brevi run`, nothing changed | 4.5 / 5.3 ms | 22.0 / 27.5 ms |
| `python3 main.py`, for scale | 16.8 / 22.0 ms | 15.0 / 16.9 ms |

`--fast` already meets the 90-day target (≤20 ms). The full check and the build do not.

## Where the time goes

`BREVI_TIME=1` on the fixture: Brevi's own front end (parse, inference, typed check, emit) finishes at **12 ms**.
Everything after that is `rustc`. Timing `rustc` by hand on the generated `main.rs` (239 lines, typed path):

| rustc step | Time |
|---|---:|
| `--emit=metadata` (what `brevi check` runs) | 70 to 82 ms |
| codegen at `opt-level=1`, 4 codegen units (the default build), objects only | 757 to 775 ms |
| the same, linked | 800 to 897 ms |
| `opt-level=0`, 16 codegen units, linked | 340 to 349 ms |
| `opt-level=0`, linked with lld | 291 to 316 ms |

So a build after an edit is about 90% LLVM optimisation. A daemon that only keeps Brevi warm saves at most the 12 ms
front end and process start; it cannot reach the 500 ms build target alone.

## Spike results

1. **rustc incremental** (`-C incremental=DIR`, one state folder per source folder, tried behind an env var and not
   merged): full check 93.6 → **68.9 ms** p50 (-26%), typed build 870 → 793 ms (-9%), dynamic build unchanged
   (it goes through `dynpy::compile`, which the trial did not touch). Worth landing for `check`; small for builds.
2. **opt-level 0 for the edit loop:** a typed build drops from about 850 ms to about **340 ms** (300 ms with lld),
   meeting the ≤500 ms target. The cost is run time: on a sieve-and-loop program the O0 binary took 474 ms, the
   O1 binary 101 ms, CPython 1040 ms. O0 is still 2x faster than CPython but 4.7x slower than today's default.

## Tiered builds (shipped in v0.27.0, `tier.rs`)

- `brevi run` and `brevi test` after an edit build at `opt-level=0` and run that binary at once.
- In the same call, a background job builds the same Rust at `opt-level=1` into the build cache, under the key the
  O1 build already uses. The next unchanged `run` (or the next test case) gets the O1 binary from the cache.
- A program that is still running after about 1 s at O0 is the case where O1 matters; agents' test runs are mostly
  start-up bound, so they gain the full 500 ms per edit.
- `brevi build` and `brevi build --release` are unchanged: what you ship is always the optimised binary.
- Keep `-C incremental` for `brevi check` (and for the O0 tier), with its state under `$BREVI_CACHE/incr` and
  evicted with the rest of the cache.

Expected after tiering: build after an edit about 350 ms typed; full check about 70 ms. That meets the 90-day build
target and leaves the check target (≤50 ms) to a resident daemon (no process start, rustc's metadata pass warm) or
to making `--fast` the default agent check (today the MCP `check` tool runs it only when called with `fast: true`).

## Go/no-go for the dev-backend spike (Cranelift or an IR interpreter)

Cranelift for rustc (`rustc_codegen_cranelift`) is nightly-only, so it cannot ship with a stable toolchain today.
Go only if, after tiered builds land:

- a typed build after an edit is still above 300 ms p50 on the fixture, **and**
- a prototype shows ≥2x on that number without failing any test in `tests/check.sh` or any conformance test that
  passes today.

Otherwise the next lever is shipping the prebuilt runtime rlib in the release archives (first build 5 to 15 s
today) and lld as the default linker where it is installed.

## Next steps

1. Land `-C incremental` for `check` with its cache eviction, behind the normal tests.
2. ~~Tiered O0/O1 builds for `run` and `test`~~: shipped in v0.27.0 (see above).
3. ~~Keep the fixture on the typed path~~: fixed in #414. #462 won back 3 of the 9 ms `check --fast` lost; the last 2 ms to the 20 ms target are next.
4. Add `bench/latency.py` to `tools/ci_local.sh` as a report (not a gate) until the numbers are stable across runs;
   then a p50/p95 non-regression gate on a fixed runner.

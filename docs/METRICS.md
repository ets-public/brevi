# Metrics

The numbers [ROADMAP.md](ROADMAP.md) steers by, defined so they cannot be gamed. Each one says what is counted, how
it is measured, and where the current value comes from. When a number is published (README, site, release notes) it
links to the artifact it came from, with the date, the Brevi version and the hardware.

## Current values

| Metric | Value | Version, date | Source |
|---|---:|---|---|
| Conformance | 80.4% (250 of 311); stdlib slice 45.1% (250 of 554) | v0.27.0, 11 Nov 2026 | `bench/conformance/report.md`, `bench/conformance/stdlib/report.md` |
| Known-wrong results | 8 tests, 16 test-mode pairs (7 and 13 on main); stdlib 20 tests, 39 pairs on main | v0.27.0, 11 Nov 2026 | `compat/conformance_known_wrong.txt`, `compat/conformance_stdlib_known_wrong.txt` |
| Crash or timeout | 0 of 311 tests (stdlib: 1) | v0.27.0, 11 Nov 2026 | `bench/conformance/report.md` |
| `check --fast` p50 / p95 | 22.0 / 24.4 ms on main (16.1 / 18.2 at v0.26.0; 160.3 / 174.4 at v0.27.0, when the fixture built dynamic) | main d91ca4b, 11 Nov 2026 | `bench/latency-results/2026-11-11-main-d91ca4b.json` |
| `check` p50 / p95 | 94.4 / 107.4 ms on main (93.6 / 113.6 ms at v0.26.0) | main d91ca4b, 11 Nov 2026 | same |
| `check`, unchanged, p50 | 2.3 ms | main d91ca4b, 11 Nov 2026 | same |
| Build after an edit, p50 | 905 ms typed, 1603 ms dynamic (`brevi run` after an edit: 348 ms, opt-level 0 tier); v0.26.0: 870 ms typed, 1508 ms dynamic | main d91ca4b, 11 Nov 2026 | same, and `...-dyn.json` |
| `run`, unchanged, p50 | 5.6 ms typed, 27.6 ms dynamic (CPython 15.3 ms) | main d91ca4b, 11 Nov 2026 | same |
| Typed-path rate | 567 of 813 (70%) | v0.25.0 | `bench/typed_corpus.py`; to be rerun on v0.26.0 |
| First-attempt native | 24/24 Haiku, 24/24 Sonnet (one run, 24 tasks, errors shown) | v0.26.0, 9 Nov 2026 | `bench/logs/e2e/se1a-*.jsonl`, `--summary` column `n1` |
| Tokens-to-green, errors shown (dollars, vs CPython) | 1.04x Haiku, 1.03x Sonnet (one run; 95% range about 0.9x to 1.1x) | v0.26.0, 9 Nov 2026 | `bench/logs/e2e/se1a-*.jsonl`, `--verdict` |
| ANER | not measured | | needs alpha traffic |
| Install to first run, no Rust | possible with `ghcr.io/ets-public/brevi-sandbox:0.27.0`; not yet timed | v0.27.0, 11 Nov 2026 | timed install test (to do) |

Latency machine: Intel Xeon @ 2.10 GHz, 4 cores, x86_64 Linux, rustc 1.97.0, CPython 3.13.18 (the same kind as v0.26.0; the v0.27.0 run was on a 2.30 GHz Xeon).

## Definitions

### Agent-Native Exactness Rate (ANER), the north star

Over a fixed corpus of programs written by models for real tasks: the share where the first `brevi check` after the
model writes the file builds natively (typed or dynamic, no CPython fallback, no retry) **and** the program's own
tests give the same stdout and exit status as CPython. Report the corpus, the model, the date and the Brevi version.
A program with no tests is left out, not counted as a match.

### Conformance

Matched / (all − unstable − excluded − by_design) on the pinned CPython tag and file list. `by_design` tests run a string as code (compile, exec, eval), which Brevi refuses on purpose; they are named in `compat/conformance_by_design.txt`, and their count is published beside the percentage. "Matched" means a native build printed
exactly what CPython printed and exited the same way; a program that ran library calls in CPython (`pybridge`) stays
matched but does not count. `tools/conformance.py` prints the file list, the CPython tag and the known-wrong count;
publish all three with the percentage. Never use "native" and "matched" as if they meant the same thing.

### Known-wrong results

Lines of `compat/conformance_known_wrong.txt`: a test id and the build mode (`normal` or `dyn`) where Brevi built
the test and gave a different answer from CPython. Count both the tests and the pairs (8 and 16 at v0.27.0). The number may only
go down: `tools/conformance.py --check` fails on any wrong result not in the file.

### Crash or timeout, and panic rate

In conformance: tests whose native build died or ran past the time limit. In fuzzing: crashes and signals per 10k
generated programs. A signal is treated as a security bug until it is triaged.

### Check latency (`bench/latency.py`)

Wall-clock from exec to exit, p50 and p95 over 20 runs on one machine, on the fixture in `bench/latency/` (three
files, 209 lines, typed path). Before each "after an edit" run the fixture's `EDIT = N` line changes, so both the
result cache and the generated-Rust cache miss, as after a real edit. Reported separately:

| Mode | Command | What it costs |
|---|---|---|
| `check_fast_edit` | `brevi check --fast` after an edit | parse, inference, guard; no `rustc` |
| `check_edit` | `brevi check` after an edit | the above plus `rustc --emit=metadata` |
| `check_cached` | `brevi check`, nothing changed | the result cache |
| `build_edit` | `brevi build` after an edit | a debug build |
| `run_cached` | `brevi run`, nothing changed | the cached binary |
| `python_run` | `python3 main.py` | CPython, for scale |

`--dyn` sets `BREVI_DYN=1` so the same program takes the dynamic build, the path most agent code takes today. Results
are kept in `bench/latency-results/DATE-VERSION[-dyn].json` with the machine they ran on. Compare numbers only from
the same machine.

### Typed-path rate

The share of programs whose final build reports `{"path":"typed"}`, over unchanged programs: the 813-program
development corpus (`bench/typed_corpus.py`) and, once it exists, alpha traffic. Report both, separately.

### First-attempt native

The share of tasks whose first `check` after the model writes the file built natively (no fallback, no retry), per
model. `bench/e2e.py` logs `natives` (one boolean per check) and `first_native` for each pyb task from 9 November
2026; `--summary` prints it as column `n1`, and `--verdict` prints it with retries per task.

### Tokens-to-green

The full API bill (input, output, cache writes and reads, every retry) for a task up to and including the model call
that ran the first passing check, with Brevi's errors shown to the model (`bench/e2e.py --show-errors`), divided by
the same for CPython over the tasks both passed. `bench/e2e.py` logs `tokens_to_green` and `usd_to_green` per task;
`--verdict` prints the ratio in tokens and in dollars. Dollars are the number that counts, since output tokens cost
5x input. The closing reply after the PASS is left out on both sides.

The three pyb arms of `bench/e2e.py`:

| Arm | When Brevi cannot build the files | What it measures |
|---|---|---|
| default | runs them with CPython; the model is not told | the release bar as defined on 2 Nov 2026 |
| `--show-errors` | runs them with CPython and shows the model Brevi's refusal | the loop an agent sees with stderr in view |
| `--native` | the check fails with Brevi's errors | the cost of insisting on a native build |

### Install to first run

Timed from a clean container with no Rust installed to `brevi run hello.py` printing `45`.

### Adoption: integrations and weekly native builds

Integrations: merged pull requests or shipped images in third-party repos that install or call Brevi. Weekly native
builds: opt-in counts from integrated platforms and the hosted service. Brevi sends no telemetry by default; any
count needs explicit consent.

## Rules

- Every public number links to its artifact, a date and the hardware.
- No absolute correctness claim. Brevi matches CPython on the programs it certifies, and `brevi verify` checks that
  on your own inputs.
- A metric is updated every release, with the old value kept, so the trend is visible.

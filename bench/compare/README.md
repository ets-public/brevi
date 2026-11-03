# Brevi vs Codon vs Nuitka

`compare.py` hands the same unchanged Python files to Brevi, [Codon](https://github.com/exaloop/codon) and [Nuitka](https://nuitka.net), and checks every built program against CPython: stdout byte for byte and the exit status (success or failure). CPython's own output is the reference, so a task's expected output does not matter here, only whether the compiled program behaves like the Python it came from.

```
python3 compare.py --corpus traps --tools brevi,brevi-dev,codon,nuitka
python3 compare.py --corpus solutions --tools brevi,codon,nuitka --jobs 4
python3 compare.py --corpus huge --tools brevi,codon,nuitka
python3 compare.py --corpus speed --tools brevi,brevi-dev,codon,nuitka   # one at a time, best of 3
python3 compare.py --report results/solutions.jsonl                     # tables
```

Corpora: `speed/` (12 programs with big inputs: the 4 from `bench/perf` plus n-body, fannkuch, spectral norm, binary trees, a set-based grid BFS, string building, objects and an LCS table), `traps/` (13 small programs on Python behaviour compilers often change: big ints, 64-bit overflow, `//` and `%` signs, float printing, dict order, unicode, exceptions and exit codes, list aliasing, deep recursion, mixed types, generators, string formatting), `solutions` (all 591 distinct saved Python solutions in `bench/solutions/*-python`, single and multi-file, run on their task's tests) and `huge` (the 18 saved huge-set solutions: chess, adventure and an interpreter in 3 to 4 files).

Tools: `brevi` is `brevi build --release`, `brevi-dev` is `brevi build` (what `brevi run` uses), `codon` is `codon build -release -exe`, `nuitka` is `python -m nuitka --follow-imports` (with ccache).

## Results (3 Nov 2026)

Brevi main 2413e64 plus this PR, Codon 0.20.6, Nuitka 4.3.4, CPython 3.13, rustc 1.97, on a 4-core Linux container.

| | Brevi | Codon | Nuitka |
| --- | --- | --- | --- |
| Saved solutions that build unchanged (591) | 589 | 206 | 591 |
| ... and match CPython on every test | 588 | 170 | 591 |
| Built but print something else (silent or crash) | 1 (stops with an error) | 36 (16 silent wrong output, 20 crashes) | 0 |
| Huge-set programs (18): build / match | 17 / 17 | 0 / 0 | 18 / 18 |
| Traps (13): match CPython | 10 | 4 | 12 |
| Median build time, small program (cold, no build cache) | 2.2 s | 4.9 s | 2.5 s (ccache warm) |
| Median run time of a test (mostly process start) | 3.4 ms | 18.6 ms | 43 ms (CPython 22 ms) |

Speed, best of 3 runs, times CPython's speed in brackets:

| Program | CPython | Brevi --release | Brevi default | Codon | Nuitka |
| --- | --- | --- | --- | --- | --- |
| nbody | 1.75s | 0.05s (38x) | 1.22s (1.4x) | 0.05s (39x) | 4.08s (0.4x) |
| fannkuch | 4.18s | 9.31s (0.4x) | 10.04s (0.4x) | 1.26s (3.3x) | 3.90s (1.1x) |
| spectral | 1.96s | 0.04s (47x) | 0.68s (2.9x) | 0.03s (75x) | 3.41s (0.6x) |
| trees | 4.28s | 1.59s (2.7x) | 4.97s (0.9x) | no build | 2.45s (1.7x) |
| set_bfs | 1.16s | 0.11s (10x) | 0.78s (1.5x) | 0.18s (6.4x) | 1.21s (1.0x) |
| strings | 1.73s | 1.26s (1.4x) | 3.78s (0.5x) | 1.82s (0.9x) | 2.14s (0.8x) |
| objects | 1.63s | 0.06s (28x) | 0.33s (4.9x) | no build | 1.77s (0.9x) |
| dp_lcs | 1.18s | 0.05s (24x) | 0.59s (2.0x) | 0.05s (22x) | 1.02s (1.1x) |
| words | 0.32s | 0.05s (6.4x) | 0.58s (0.5x) | 0.18s (1.8x) | 0.28s (1.1x) |
| graph | 2.38s | 0.43s (5.6x) | 1.46s (1.6x) | 0.37s (6.4x) | 2.57s (0.9x) |
| sieve | 0.84s | 0.08s (10x) | 0.48s (1.7x) | 0.03s (30x) | 0.75s (1.1x) |
| records | 0.51s | 0.32s (1.6x) | 1.76s (0.3x) | 0.34s (1.5x) | 0.64s (0.8x) |

Every built speed program printed what CPython prints. Nuitka ran slower than CPython on several here; this container's python3 is an optimised build, while Nuitka links its own libpython, so treat its column as this machine's result, not a general one.

## What it shows

- **Correctness.** Brevi and Nuitka run nearly every agent-written program exactly as CPython does; Codon builds about a third of them, and of those, 1 in 6 prints something else, often silently (64-bit ints wrap, `json` and dict-of-mixed-values programs print 0, `print` output is lost when the program crashes, exceptions print without quotes). Brevi's failures are loud: a refused build or an error naming the line.
- **Speed.** On typed code Brevi `--release` matches Codon (nbody, spectral, dp_lcs, graph, set_bfs) and is far ahead of Nuitka, which stays near CPython. Brevi also runs object-heavy code Codon refuses (`objects` 28x, `trees` 2.7x).
- **Gaps** (on the roadmap): `brevi run`'s default build is 3x to 20x slower than `--release` and slower than CPython on 5 of 12 programs; list slice assignment (`perm[:k+1] = perm[k::-1]`) is refused by the typed build, so fannkuch runs in the dynamic build at 0.4x CPython; typed ints stop with an overflow error where Python grows a big int (`f *= i` for a factorial, `2 ** 100`); generators are not in dynamic builds yet (one huge-set chess program and the generators trap); `__file__`; csv.DictReader rows used as dict keys guess int keys.

Brevi bugs this comparison found, fixed with it: `book = asks if buy else bids` then `book.remove(o)` changed a copy; `m.group(3) is None` was False for an unmatched group (also when the test sits in an imported module: the typed-build guard now reads local imports too); an f-string value starting with a number printed as text; `raise SystemExit(3)` gave the wrong exit status; `d.setdefault(k, set()).add(x)` raised KeyError; `10 ** -2` was refused.

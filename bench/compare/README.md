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

Brevi main d7fe346 (after #127 and #128), Codon 0.20.6, Nuitka 4.3.4, CPython 3.13, rustc 1.97, on a 4-core Linux container. Codon and Nuitka figures are from the first run (main 2413e64), against CPython timed in that run.

| | Brevi | Codon | Nuitka |
| --- | --- | --- | --- |
| Saved solutions that build unchanged (591) | 591 | 206 | 591 |
| ... and match CPython on every test | 591 | 170 | 591 |
| Built but behave differently | 0 | 36 (16 silent wrong output, 20 crashes) | 0 |
| Huge-set programs (18): build / match | 18 / 18 | 0 / 0 | 18 / 18 |
| Traps (13): match CPython | 13 | 4 | 12 |
| Median cold build of a small program, default / --release | 1.8 s / 3.1 s | 4.9 s | 2.5 s (ccache warm) |
| Median run time of a test (mostly process start) | 3.4 ms | 18.6 ms | 43 ms (CPython 22 ms) |

Both Brevi builds (default and `--release`) give the same correctness figures. Speed, best of 3 runs, times CPython's speed in brackets:

| Program | CPython | Brevi default (`brevi run`) | Brevi --release | Codon | Nuitka |
| --- | --- | --- | --- | --- | --- |
| spectral | 2.67 s | 0.07 s (39.5x) | 0.06 s (44.5x) | 0.03 s (74.6x) | 3.41 s (0.6x) |
| nbody | 2.33 s | 0.11 s (20.8x) | 0.08 s (29.3x) | 0.05 s (38.5x) | 4.08 s (0.4x) |
| objects | 1.97 s | 0.11 s (18.4x) | 0.07 s (29.7x) | no build | 1.77 s (0.9x) |
| dp_lcs | 1.94 s | 0.14 s (14.0x) | 0.06 s (32.2x) | 0.05 s (21.9x) | 1.02 s (1.1x) |
| sieve | 1.06 s | 0.10 s (10.7x) | 0.08 s (13.0x) | 0.03 s (30.3x) | 0.75 s (1.1x) |
| set_bfs | 1.40 s | 0.15 s (9.6x) | 0.13 s (10.6x) | 0.18 s (6.4x) | 1.21 s (1.0x) |
| trees | 5.29 s | 0.71 s (7.4x) | 0.96 s (5.5x) | no build | 2.45 s (1.7x) |
| words | 0.34 s | 0.06 s (5.6x) | 0.06 s (5.7x) | 0.18 s (1.8x) | 0.28 s (1.1x) |
| graph | 2.34 s | 0.44 s (5.3x) | 0.38 s (6.1x) | 0.37 s (6.4x) | 2.57 s (0.9x) |
| fannkuch | 5.37 s | 1.61 s (3.3x) | 1.26 s (4.3x) | 1.26 s (3.3x) | 3.90 s (1.1x) |
| strings | 2.36 s | 1.07 s (2.2x) | 0.82 s (2.9x) | 1.82 s (0.9x) | 2.14 s (0.8x) |
| records | 0.68 s | 0.39 s (1.7x) | 0.26 s (2.6x) | 0.34 s (1.5x) | 0.64 s (0.8x) |

Every built speed program printed what CPython prints. Nuitka ran slower than CPython on several here; this container's python3 is an optimised build, while Nuitka links its own libpython, so treat its column as this machine's result, not a general one.

## What it shows

- **Correctness.** Brevi and Nuitka run every agent-written program exactly as CPython does. Codon builds about a third of them, and of those, 1 in 6 prints something else, often silently (64-bit ints wrap, `json` and dict-of-mixed-value programs print 0, `print` output is lost when the program crashes, exceptions print without quotes).
- **Speed.** Even `brevi run`'s default build beats CPython on all 12 programs (1.7x to 40x); `--release` is level with Codon where Codon builds, and also runs object-heavy code Codon refuses. Nuitka stays near CPython.
- **Open gap:** a big int that grows inside a function and is returned still stops with an overflow error (loud, not wrong).

History: the first run (main 2413e64) found six Brevi bugs, four of them silent wrong answers, fixed in #127; its follow-ups (fast default build, typed slice assignment, big-int growth, generators in dynamic builds, faster small objects and strings, csv.DictReader, `__file__`) landed in #128. Before them, the default build was slower than CPython on 5 of 12 programs and Brevi matched 588 of 591 solutions and 17 of 18 huge-set programs.

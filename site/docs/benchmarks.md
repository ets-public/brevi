# Benchmarks
<!-- description: Brevi's release test with Claude models, model cost against CPython, heavy-task results, and the comparison with Codon and Nuitka. -->

Brevi is measured two ways: **what it costs an AI model** to reach a working program, and **how the built programs behave**, against CPython and against the other Python compilers. All benchmarks, logs and scripts are in the repository's `bench/` folder, so every number here can be rerun. Real programs built with Brevi, such as an MCP server, are timed on [Showcases](/docs/showcases).

## The release test

The release test is the bar every release has to pass. Claude Sonnet and Claude Haiku each solve the 24 tasks of the `rel` suite twice per side:

- **python:** the model writes Python and checks it with CPython until the tests pass;
- **pyb:** the same prompt, and the same Python, checked with Brevi.

Each side's prompt cache is kept apart so neither pays the other's cache writes. The whole API bill is counted, retries included. Since 3 November every Brevi check also runs the same files with CPython (the "shadow check"), so a check that failed only because of Brevi is counted as a Brevi bug.

**The bar:** every task passes, Brevi's cost is within 10% of CPython's averaged over the runs for both models, and at least 90% of final programs build natively, with no CPython fallback.

| Runs (Nov 2026) | Sonnet cost, Brevi vs CPython | Haiku cost, Brevi vs CPython | Tasks passed | Built natively | Failed because of Brevi |
| --- | --- | --- | --- | --- | --- |
| r270a, r270b (v0.27.0, two runs, 11 Nov) | **1.08x** (95% range 1.04x to 1.13x) | **0.97x** (0.90x to 1.03x) | 24 of 24 in every run, both sides | 46 of 48 and 46 of 48 (the 4 others were build failures while another job used the same 4 cores; each builds natively when rebuilt) | 0 |
| r260a, r260b (v0.26.0, two runs, 9 Nov) | **1.07x** (95% range 1.04x to 1.11x) | **1.00x** (0.95x to 1.04x) | 24 of 24 in every run, both sides | 48 of 48 and 48 of 48 | 0 |
| r250c, r250d (v0.25.0, two runs, 4 Nov) | **1.09x** (95% range 1.03x to 1.15x) | **1.01x** (0.94x to 1.09x) | 24 of 24 in every run with Brevi; one Haiku run with CPython missed a task | 47 of 48 and 48 of 48 | 0 |
| r250a, r250b (main just before v0.25.0's last fix, two runs) | **1.04x** (95% range 1.00x to 1.08x) | **1.05x** (1.00x to 1.10x) | 24 of 24 in every run, both sides | 48 of 48 and 48 of 48 | 0 |
| r246a, r246b (v0.24.6, two runs, 4 Nov) | **1.03x** (95% range 0.97x to 1.09x) | **1.02x** (0.97x to 1.08x) | 24 of 24 in every run, both sides | 47 of 48 and 48 of 48 | 0 |
| r245a, r245b (v0.24.5, two runs) | **1.06x** (95% range 1.01x to 1.12x) | **1.01x** (0.95x to 1.07x) | 24 of 24 in every run, both sides | 48 of 48 and 47 of 48 | 0 |
| r244a to r244e (v0.24.4, five runs) | **1.06x** (95% range 1.03x to 1.10x) | **1.03x** (0.98x to 1.10x) | 24 of 24 in every run, both sides | 120 of 120 and 117 of 120 | 5 checks, final programs all passed |
| w163a (main after v0.24.3, one run) | **1.06x** | **0.96x** | 24 of 24, both sides | 22 of 24 and 23 of 24 | 0 |
| w156a (v0.24.2, one run) | **1.09x** | **1.06x** | 24 of 24, both sides | 23 of 24 each | 0 |
| v1a, v1b (v0.24.0) | **1.07x** | **1.00x** | 24 of 24, both sides | 45 of 48 and 47 of 48 | 0 |
| u2a, u2b | **0.97x** (95% range 0.93x to 1.01x) | **1.01x** (0.96x to 1.05x) | 24 of 24, both sides | 44 of 48 and 46 of 48 | 0 |

**Passed** in every row. In `r244a` to `r244e` (`r244e` on the release commit, the rest on main just before it), five checks (in the `trains` and `poker` tasks) failed under Brevi where CPython passed the same files: the typed build printed a wrong answer or stopped with an error. Each model fixed its code and passed; the cases are saved and being fixed for the next release. The one-run checks after v0.24.0 are rerun after each change to the compiler that could move cost; the two fallbacks left in `w156a` were both `json_query`, whose `json.JSONDecoder().raw_decode` builds natively on main since then. The earlier runs e31 to e34 (2 November) also passed: 48 of 48 tasks, all 48 final programs native, cost 1.10x (Sonnet) and 0.95x (Haiku).

Read these as "about the same cost", not a saving: across releases Sonnet's ratio has mostly sat a few percent above 1.00x, the 95% ranges come from resampling the 24 tasks (not runs, of which there are two), and in these runs a file Brevi cannot build runs with CPython without the model seeing Brevi's build error. The bar is "no more expensive than Python", not "cheaper", on purpose: on ordinary tasks a model writes much the same Python either way, so the cost should come out level, and the speed and safety of the result come free. The test exists to catch Brevi making an agent work harder, by rejecting valid Python or answering differently.

## Weekly agent-code benchmark

<!-- weekly:start -->

Each week Claude Sonnet, Claude Haiku and DeepSeek V4.1 Flash solve the 24 tasks of the `rel` suite twice in plain Python: once checked with CPython, once checked and built with Brevi, with the same prompt. The whole API bill is counted, retries included. **Ratio** is Brevi's cost over CPython's (below 1.00x is cheaper). **Programs run** is the time the finished programs took on their tests, over the tasks both sides finished, and **Speed** is CPython's time over Brevi's. These test inputs are small, so most of each figure is process start-up; the [heavy tasks](#heavy-tasks-and-the-plain-solution-line) show the gap on real work. The data is [bench/weekly.json](repo:bench/weekly.json) and every run's log is in [bench/logs/e2e](repo:bench/logs/e2e).

| Date | Brevi | Model | Cost per run, Brevi / CPython | Ratio | Tasks finished, Brevi / CPython | Built natively | Programs run, Brevi / CPython | Speed |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 2026-11-09 | v0.26.0 | Sonnet 5.5 | $0.746 / $0.698 | **1.07x** | 48/48 / 48/48 | 47/48 | 1.51 s / 4.56 s | 3.02x |
| 2026-11-09 | v0.26.0 | Haiku 5.5 | $0.084 / $0.084 | **1.01x** | 48/48 / 48/48 | 48/48 | 0.56 s / 4.49 s | 8.0x |
| 2026-11-09 | v0.26.0 | DeepSeek V4.1 Flash | $0.377 / $0.314 | **1.20x** | 47/48 / 47/48 | 45/48 | 0.76 s / 4.53 s | 5.94x |
| 2026-11-04 | v0.25.0 | Sonnet 5.5 | $0.729 / $0.671 | **1.09x** | 48/48 / 48/48 | 47/48 | not timed | - |
| 2026-11-04 | v0.25.0 | Haiku 5.5 | $0.086 / $0.085 | **1.01x** | 48/48 / 47/48 | 48/48 | not timed | - |
<!-- weekly:end -->

## Heavy tasks and the plain-solution line

The `speed` set has four tasks with small inputs and tens of millions of steps under a 10-second limit: Game of Life up to 600x600, 0/1 knapsack, edit distance on 10,000-letter texts, and Collatz chains. A plain Python solution takes CPython 15 to 80 seconds, so a model writing for CPython has to hand-optimise.

With one extra line in the prompt telling the model its code is compiled and to write the plain solution (see [For AI agents](/docs/agents#write-the-plain-solution)):

| Runs sp2a, sp2b (3 Nov 2026) | CPython, model told nothing | Brevi, plain-solution line | Ratio |
| --- | --- | --- | --- |
| Sonnet | $0.222 | $0.128 | **0.58x** |
| Haiku | $0.0221 | $0.0117 | **0.53x** |

Every task passed on both sides, every Brevi program built natively, and no check failed because of Brevi. The saving is the model not hand-optimising: output tokens fell by about half for Sonnet and a quarter for Haiku. **This is not yet a fair test of Brevi:** only the Brevi side got the extra line, and CPython has not been run with the same line, so part or all of the saving may come from the prompt. The line also said "10 to 50 times faster", which overstated the measured speed-up; it now gives the measured range. That control has now been run (`e2e.py --lang pyh`: the Python prompt with the same line, checked with CPython), two runs per model on 9 November 2026 against the same Brevi build:

| Runs sp3a, sp3b (9 Nov 2026) | CPython, same plain-solution line | Brevi, plain-solution line | Ratio |
| --- | --- | --- | --- |
| Sonnet | $0.159 | $0.073 | **0.46x** |
| Haiku | $0.0154 | $0.0052 | **0.34x** |

With the same line on both sides the saving is Brevi's, not the prompt's: told to write the plain solution, the model does, and on CPython that solution runs over the 10-second limit, so it spends 10 to 12 checks per run (and 2 to 3 times the output tokens) making the code fast; Haiku gave up on one task in one run. On Brevi the plain solution passed first time in 15 of 16 cases (all 16 built natively, 15 on the typed build). Four tasks and two runs per model is a small sample: the per-run costs were $0.170 and $0.149 against $0.072 and $0.074 for Sonnet, $0.0126 and $0.0182 against $0.0052 and $0.0052 for Haiku. The logs are `sp3a-*` and `sp3b-*` in [bench/logs/e2e](repo:bench/logs/e2e). On the ordinary release-suite tasks the same line made no clear difference (runs v1a, v1b: Sonnet 1.07x, Haiku 1.00x, bar still met).

## Brevi, Codon and Nuitka

The same unchanged Python files were given to each compiler, and every built program was checked against CPython: stdout byte for byte and the exit status. Brevi is `brevi build --release`; Codon is `codon build -release -exe`; Nuitka is `python -m nuitka --follow-imports`.

| Same result as CPython | Brevi | Codon | Nuitka |
| --- | --- | --- | --- |
| Saved agent programs that build unchanged (591; our own solutions, kept once they passed under Brevi) | **591** | 206 | 591 |
| ... and match CPython on every test | **591** | 170 | 591 |
| Built but behave differently | **0** | 36 of the 206 built (16 silent wrong output, 20 crashes) | 0 |
| Large programs (18): build / match | **18 / 18** | 0 / 0 | 18 / 18 |
| Behaviour traps (13) | **13** | 4 | 12 |
| Median cold build of a saved program, no cache | 1.2 s (default), 3.1 s (`--release`) | 4.9 s | 2.5 s (ccache warm) |
| Median run time of a test (mostly start-up) | **3.4 ms** | 18.6 ms | 43 ms (CPython 22 ms) |

Speed, best of 3 runs, in seconds:

| Program | CPython | Brevi default | Brevi `--release` | Codon | Nuitka |
| --- | ---: | ---: | ---: | ---: | ---: |
| Spectral norm | 2.67 | 0.07 | 0.06 | **0.03** | 3.41 |
| N-body | 2.33 | 0.11 | 0.08 | **0.05** | 4.08 |
| Objects | 1.97 | 0.11 | **0.07** | no build | 1.77 |
| LCS table | 1.94 | 0.14 | 0.06 | **0.05** | 1.02 |
| Prime sieve | 1.06 | 0.10 | 0.08 | **0.03** | 0.75 |
| Grid BFS with a set | 1.40 | 0.15 | **0.13** | 0.18 | 1.21 |
| Binary trees | 5.29 | **0.71** | 0.96 | no build | 2.45 |
| Word counts | 0.34 | 0.06 | **0.06** | 0.18 | 0.28 |
| Dijkstra | 2.34 | 0.44 | 0.38 | **0.37** | 2.57 |
| Fannkuch | 5.37 | 1.61 | **1.26** | **1.26** | 3.90 |
| String building | 2.36 | 1.07 | **0.82** | 1.82 | 2.14 |
| Group records | 0.68 | 0.39 | **0.26** | 0.34 | 0.64 |

On these 3 November timings Brevi's default build (what `brevi run` uses) was 1.7x to 39.5x faster than CPython, median 8.5x; `--release` was 2.6x to 44.5x, median about 8.4x. The newer figures from 12 November, on a different machine, are in the next section. Codon is faster on the numeric programs (spectral norm, n-body, sieve, LCS) and Brevi on objects, strings, sets and records; Brevi builds programs Codon cannot. Nuitka matches CPython's behaviour like Brevi and runs near CPython's speed overall, from about 2x faster (binary trees) to about 2x slower (n-body, likely a container effect) here. These 12 are compute-heavy programs, the kind a compiler helps most, and they build on Brevi's typed build; a program on the dynamic build runs much closer to CPython's speed. Measured 3 November 2026 on one 4-core Linux machine (Codon 0.20.6, Nuitka 4.3.4, CPython 3.13, rustc 1.97). The Codon and Nuitka times are from an earlier session on that machine than the Brevi and CPython times ([bench/compare](repo:bench/compare/README.md)), so compare them loosely.

## Brevi and CPython, 12 November 2026

Re-measured alone on a Ryzen 9 5950X (32 threads) under WSL2 Ubuntu 24.04, Brevi main b1004c2 (0.27.1 plus merged work), CPython 3.13.13, rustc 1.99.1, best of 5 runs, one program at a time, in seconds with the speed-up over CPython in brackets. All 12 programs printed what CPython prints, on both the typed and the dynamic build. Codon and Nuitka were not rerun.

| Program | CPython | Brevi default | Brevi `--release` | Dynamic build |
| --- | ---: | ---: | ---: | ---: |
| Objects | 1.29 | 0.03 (37.9x) | 0.03 (38.8x) | 0.90 (1.4x) |
| Spectral norm | 1.87 | 0.06 (31.0x) | 0.05 (34.2x) | 2.17 (0.9x) |
| LCS table | 1.12 | 0.04 (28.9x) | 0.04 (31.4x) | 1.67 (0.7x) |
| N-body | 2.00 | 0.09 (22.5x) | 0.08 (26.4x) | 3.70 (0.5x) |
| Grid BFS with a set | 0.79 | 0.06 (12.9x) | 0.06 (14.2x) | 0.75 (1.0x) |
| Prime sieve | 0.49 | 0.04 (12.0x) | 0.04 (11.7x) | 0.46 (1.1x) |
| Binary trees | 3.05 | 0.46 (6.6x) | 0.39 (7.8x) | 2.45 (1.2x) |
| Dijkstra | 1.24 | 0.19 (6.4x) | 0.18 (7.0x) | 1.24 (1.0x) |
| Word counts | 0.23 | 0.04 (5.3x) | 0.05 (5.0x) | 0.32 (0.7x) |
| Fannkuch | 3.55 | 1.29 (2.8x) | 1.01 (3.5x) | 8.00 (0.5x) |
| String building | 1.36 | 0.69 (2.0x) | 0.53 (2.6x) | 1.68 (0.8x) |
| Group records | 0.45 | 0.27 (1.6x) | 0.21 (2.1x) | 0.84 (0.5x) |
| **Median** | | **9.3x** | **9.8x** | **0.85x** |

The ranges are 1.6x to 37.9x (default), 2.1x to 38.8x (`--release`) and 0.5x to 1.4x (dynamic). The dynamic build (`BREVI_DYN=1`, with `--release`) is what a program whose values change type gets; it runs at about CPython's speed, slower on numeric loops. Median cold build: 0.2 s default, 1.1 s `--release`, 0.3 s dynamic. The 3 November figures came from a different machine, so the two sets are not a like-for-like comparison.

Library and dynamic-path cases (`bench/speed/gaps`, median of 3): polars on a large frame 2.0x faster than CPython; an asyncio loop 1.2x slower; sqlite point queries 1.3x slower; a Jinja chat template 2.9x slower; bulk pydantic validation 10.2x slower. A small autograd program shaped like microgpt (100 steps) ran 1.18x faster than CPython with 13% more memory.

## Rerunning

```
cd bench
python3 e2e.py RUNa --lang pyb --model sonnet --set rel --jobs 4      # the release test (needs an Anthropic API key)
python3 e2e.py RUNa --lang python --model sonnet --set rel --jobs 4
python3 e2e.py --verdict "logs/e2e/RUN[a-d]-*.jsonl"
cd ..
python3 bench/compare/compare.py --corpus solutions --tools brevi,codon,nuitka --jobs 4
python3 bench/compare/compare.py --corpus speed --tools brevi,brevi-dev,codon,nuitka
```

`bench/README.md` has the full method and every run since the project began, including older token-count benchmarks of Brevi's own `.bv` language against Python and Rust.<!--public--> Browse it on [GitHub](repo:bench/README.md).<!--/public-->

## The history, briefly

Brevi began as a language of its own for models to write, measured by tokens per finished task against Python. It got close (within a few percent either way on most sets), but the measurements kept showing that models write Python more cheaply and more reliably than any new syntax, however short. So the goal moved: take the Python models already write, and make it fast and safe. The `.bv` language is still there for anyone who wants it.

# Brevi in numbers

<!-- Written by `python3 tools/published_numbers.py --write --tag v0.28.1` from the tag's own conformance reports and bench/compare/speed.json. Never edit by hand: CI (`tools/published_numbers.py --check`) refuses a copy that differs. -->

The one table every page, release note and site quotes. Release **v0.28.1** (commit `96a51afcd5e4`). The reports measured commit `aa554040b5e3`; v0.28.1 tags `96a51afcd5e4`, which adds only the conformance reports and floors to it (4 files: `git diff --stat aa55404 96a51af`), so the numbers describe exactly the released code.

## Conformance: CPython 3.13's own tests

Native builds of CPython 3.13.13's own tests, Linux, at `PYTHONHASHSEED=0`. Each scored test must print exactly what CPython prints. Reports: [core](https://github.com/ets-public/brevi/blob/v0.28.1/bench/conformance/report.md), [standard library](https://github.com/ets-public/brevi/blob/v0.28.1/bench/conformance/stdlib/report.md).

| | Core language | Standard library |
| --- | ---: | ---: |
| Tests in the slice | 1,337 | 2,231 |
| Refused by design (run a string as code), excluded | 83 | 6 |
| Not scored: output unstable between CPython runs, or excluded by the harness | 182 | 172 |
| **Scored** | **1,072** | **2,053** |
| **Match CPython, native build** | **1,043 of 1,072** (97.3% of the scored suite, with 83 string-as-code tests excluded) | **1,989 of 2,053** (96.9% of the scored suite, with 6 string-as-code tests excluded) |
| Give a different answer | 0 | 0 |
| Refused at build time (count as not matched) | 26 | 51 |
| Could not be extracted (count as not matched) | 3 | 11 |
| Matched only through the CPython bridge (not counted) | 0 | 2 |
| Matches that print something to compare (assertion-bearing) | 945 of 1,072 (88.2%) | 1,937 of 2,053 (94.3%) |
| Matches that print nothing (built and exited 0) | 98 | 53 |
| Matches that took the typed build (typed share) | 27 of 1,043 (2.6%) | 23 of 1,989 (1.2%) |
| Forced dynamic build (`BREVI_DYN=1`): match | 1,043 of 1,072 (97.3% of the scored suite, with 83 string-as-code tests excluded) | 1,991 of 2,053 (97.0% of the scored suite, with 6 string-as-code tests excluded) |

**How to quote the headline:** 97.3% of the scored suite, with 83 string-as-code tests excluded (1,043 of 1,072). The standard library slice: 96.9% of the scored suite, with 6 string-as-code tests excluded. Never quote a coverage share without its scope; CI refuses a page that does.

The core report at v0.28.1 prints the assertion-bearing share as 88.1%: it rounded twice (a fix in tools/share.py rounds once, from the counts). 945 of 1,072 is 88.2%, the figure to quote.

Which build the conformance programs took (normal build, every program, not just matches): core 1,282 programs: typed 27 of 1,282 (2.1%), dynamic 1,146 of 1,282 (89.4%), pybridge 0, refused 109; stdlib 2,049 programs: typed 23 of 2,049 (1.1%), dynamic 1,967 of 2,049 (96.0%), pybridge 2, refused 57.

## Speed: typed build against dynamic build

bench/compare's 12 compute programs, speed-up over CPython 3.13.13 (best of 5, one at a time). Measured 2026-11-12 on Brevi 0.27.1 (`b1004c2`), AMD Ryzen 9 5950X (32 threads), WSL2 Ubuntu 24.04, 15 GB; run alone; not yet re-measured on v0.28.1. Data: [bench/compare/speed.json](../bench/compare/speed.json).

| | Typed, default build | Typed, `--release` | Dynamic (`BREVI_DYN=1 --release`) |
| --- | ---: | ---: | ---: |
| **Median** | **9.3x** | **9.8x** | **0.85x** |
| Range | 1.6x to 37.9x | 2.1x to 38.8x | 0.5x to 1.4x |
| Slower than CPython | 0 of 12 | 0 of 12 | 7 of 12 |

**How to quote the speed:** typed median 9.3x, dynamic median 0.85x. The 9.3x figure holds only on the typed path: all 12 of these programs build typed, but only 2.6% of core and 1.2% of standard-library conformance matches took the typed build. A program whose values change type gets the dynamic build, which runs at about CPython's speed or slower. CI refuses a page that quotes a typed median without the dynamic one.

## Windows

The v0.28.1 release gate held the core slice on the shipped `brevi.exe` (at least 90%, no wrong answer; Windows floor 1,042 native matches). The standard-library slice was not gated on Windows in v0.28.1; its limits are in [KNOWN_ISSUES.md](KNOWN_ISSUES.md#windows-beta).

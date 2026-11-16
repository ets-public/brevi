# Speed by build path: a case study on agent-written Python

Brevi builds a Python file one of three ways: the **typed** build (Rust with native types, the fast one), the **dynamic** build (a native binary with Python's object model, for programs whose values change type), or **pybridge** (the program runs through an embedded CPython). Which one a program gets decides how fast it runs, so a single speed figure says little without the share of programs that get it. This page measures both on one real workload: Python that models wrote to finish benchmark tasks.

## The short answer

On 587 distinct agent-written programs, built with Brevi from the **v0.28.2** release tag (commit `66d196da`):

| | Typed | Dynamic | Pybridge | Refused |
| --- | ---: | ---: | ---: | ---: |
| Programs that took the path | **366 of 587 (62.4%)** | **221 of 587 (37.6%)** | 0 | 0 |
| Printed what CPython printed, every test | 365 | 221 | - | - |
| Stopped with the typed build's checked error | 1 | 0 | - | - |
| Wrong answers | **0** | **0** | - | - |
| Median time per test, CPython | 11.7 ms | 17.3 ms | - | - |
| Median time per test, native binary | 1.1 ms | 2.3 ms | - | - |
| Whole-run speed-up over CPython, median | 10.8x | 7.4x | - | - |
| Range | 8.5x to 18.6x | 3.1x to 9.0x | - | - |

**Read the speed rows as start-up, not compute.** These programs get small test inputs: CPython needs about 10 to 12 ms just to start and print one line on this machine, so most of its 11.7 ms and 17.3 ms per test is the interpreter starting and importing modules. A native binary starts in about 1 ms (typed) or 2 ms (dynamic). The speed-ups above are what an agent sees when it runs many short programs; they are not how fast either build computes.

**Compute speed by path** comes from the released numbers ([NUMBERS.md](NUMBERS.md), bench/compare's 12 compute programs, measured on Brevi 0.27.1 and not yet re-measured on v0.28.x): typed median 9.3x, dynamic median 0.85x against CPython. A long-running program on the dynamic build runs at about CPython's speed, slower on numeric loops; only the typed build is several times faster.

The one program that stopped (`n1-sonnet-python/stack_vm.py`) masks numbers with `(1 << 64) - 1`, which does not fit the typed build's 64-bit integers. Its binary printed nothing, gave the error `the typed build stopped: an int passed 64 bits` and exited 1, so it is not a wrong answer: `brevi run` re-runs such a program on the dynamic build, and `BREVI_DYN=1` builds it that way.

## The typed share falls as programs grow

| Program size (lines of Python, all files) | Took the typed build |
| --- | ---: |
| Under 40 | 205 of 250 (82.0%) |
| 40 to 79 | 105 of 132 (79.5%) |
| 80 to 149 | 53 of 133 (39.8%) |
| 150 or more | 3 of 72 (4.2%) |

The median program here is 48 lines. Short, single-purpose task solutions mostly build typed; programs of a few hundred lines, and 30 of the 32 multi-file programs, mostly do not. The same pattern shows in Brevi's other samples, so the typed share depends heavily on what is measured:

- The final programs of the weekly agent benchmark on v0.28.2 (`w1116a` and `w1116b`, Sonnet, Haiku and DeepSeek on the 24 `rel` tasks, [logs](../bench/logs/e2e)): typed 45 of 143 (31.5%), dynamic 98, pybridge 0.
- CPython 3.13's own conformance tests at v0.28.1 ([NUMBERS.md](NUMBERS.md)): typed 2.1% of core programs and 1.1% of standard-library programs; nearly all the rest built dynamic.

So the typed path covers most small agent scripts but only a small fraction of larger programs and of CPython's test suite.

## Why programs leave the typed build

The build prints its first reason. Grouped, over the 221 dynamic programs:

| Programs | Reason |
| ---: | --- |
| 57 | Two names share one list and one of them is changed while the other is still used (the typed build would change a copy) |
| 40 | A loop, comprehension or `str()` over a set: CPython walks it in hash order, which the typed build would not reproduce |
| 30 | `is` between values other than `None`, `True`, `False` and `Ellipsis` needs object identity |
| 29 | The typed build's generated Rust did not compile |
| 19 | A value changes type |
| 19 | A library or method the typed build does not have (`datetime`, `decimal`, `re.findall` with groups, ...) |
| 2 | An int past 64 bits |
| 25 | Other: a type the build could not work out, a name defined in a way it does not follow, a `return` out of `try`/`finally`, `nonlocal` on a list, two parse gaps |

Apart from the compile failures, each is the typed build declining rather than guessing: when it cannot show it prints what CPython prints, the program goes to the dynamic build. The 29 Rust compile failures are compiler gaps, caught at build time and sent to the dynamic build too; they are the cheapest typed share to win back.

## How it was measured

- **Workload:** every Python solution a model saved in Brevi's end-to-end benchmark ([bench/solutions](../bench/solutions), `*-python` folders: Claude Sonnet and Haiku across 32 rounds, one file per task or a folder with `main.py`), run on that task's tests ([bench/tasks](../bench/tasks)). Identical programs saved twice count once: 591 distinct programs on 60 tasks. The 4 whose own output CPython gets wrong on the task's tests are left out, leaving 587.
- **Builds:** `brevi build` with `BREVI_PATH=1`, which prints the path each build took. Build time is not counted.
- **Runs:** CPython 3.13.18 and each native binary run every test with the same input, one at a time, best of 5, at `PYTHONHASHSEED=0`. A program's time is the sum over its tests. Output and exit status have to match CPython's on every test, or the program counts as a stop or a wrong answer, never as a speed-up.
- **Machine:** a 4-core cloud Linux container (Intel Xeon at 2.30 GHz). Timings are noisy there: CPython's start-up floor measured 10.2 ms, 10.8 ms and 12.3 ms in three runs of the same script. The path shares and the zero wrong answers do not depend on the machine.
- **Safety:** model-written code runs only inside [tools/modelbox.py](../tools/modelbox.py) (no network, no credentials).
- **Data:** [bench/pathsplit/v0.28.2.json](../bench/pathsplit/v0.28.2.json), one record per program with its path, the build's reason, and both timings.

Rerun it with a brevi built from a release tag (on Linux, as root, for the box):

```bash
git checkout v0.28.2 && cargo build --release && mkdir -p target/rel && cp target/release/brevi target/rel/
git checkout main
BREVI=target/rel/brevi python3 bench/pathsplit.py target/pathsplit.json
python3 bench/pathsplit.py --report target/pathsplit.json
```

## Limits

- These solutions were written in earlier benchmark rounds, against earlier Brevi versions and prompts; the models did not see v0.28.2. They are real agent-written Python, but of the short task kind: no program here runs long enough to measure compute speed.
- The test inputs are small, so the whole-run speed-ups measure start-up. A program that does seconds of work on the dynamic build should be expected near CPython's speed (dynamic median 0.85x in the compute benchmark), not 7.4x.
- Linux only; nothing here was measured on Windows ([KNOWN_ISSUES.md](KNOWN_ISSUES.md#windows-beta) has its limits).

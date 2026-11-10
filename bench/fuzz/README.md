# Fuzzing

`tools/fuzz.py` writes seeded random Python in the style agents write, runs each program under CPython, builds it
with `brevi build` on the normal path and with `BREVI_DYN=1`, and compares. A Rust panic, a signal or a compiler
crash is a failure; a wrong answer is reported. A failing batch is cut down to the one case that fails, saved to
`found/` (not committed) with its seed, and `reduce()` in the tool shrinks a case further.

- `corpus/`: cases `--replay` reruns on every pull request. Each must match CPython on both builds. Add the reduced
  case here in the PR that fixes a fuzz finding, so it cannot come back. The `smoke_*` cases are ordinary generated
  programs that matched when the corpus was started.
- The daily CI run fuzzes 60 fresh batches of 12 cases, seeded by the date.

    python3 tools/fuzz.py --batches 100 --seed 7 --jobs 4
    python3 tools/fuzz.py --replay bench/fuzz/corpus
    python3 tools/fuzz.py --show 7:3     # print batch 3 of seed 7

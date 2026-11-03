---
name: brevi
description: Build and run Python programs as fast native binaries with Brevi, or write Brevi (.bv) programs. Use when a small fast command-line program is wanted, when a task mentions Brevi, or when a .bv file is involved.
---

# Brevi

Brevi compiles ordinary Python to Rust and builds a native binary. Write the program in plain Python, as you normally would; Brevi reports anything it cannot build as one short JSON line with a suggested fix.

## Setup

Check that `brevi --version` works. If it does not, run the repository's `install.sh`: it fetches the Rust toolchain if `rustc` is missing (Brevi compiles through it), then a prebuilt `brevi` from the latest release, or builds it with `cargo install --git https://github.com/ets-public/brevi`.

## Loop

1. Write normal Python to `main.py` (standard library or installed packages such as numpy; split big programs into `.py` files that import each other). There is no guide to read first. Brevi compiles it to native code, which runs plain loops over ints, lists and strings 10 to 50 times faster than CPython: write the plain, direct solution and do not hand-optimise it for speed (no bit-packing tricks or big-int bitboards).
2. Run it with `brevi run main.py < input.txt`, or `brevi check main.py` to build without running. `brevi build main.py -o app --release` makes the fast binary.
3. Each error is `{"line":N,"msg":...,"fix":...}`. Change only the lines it names, then run again. Do not rewrite the whole file.
4. If Brevi still cannot build a Python construct after one small change, keep the Python as it is, run it with `python3 main.py`, and say that it ran without Brevi.

## Writing Brevi itself

Only for `.bv` files or when Brevi syntax is asked for: run `brevi guide` once and read the guide it prints; `brevi guide WORD` prints the lines about one name. `brevi patch NAME.bv patch.bv` replaces whole functions by name, `brevi fix NAME.bv` applies suggested renames, and `brevi guide Libraries` covers Rust crates (`rust fn`) and Python (`py fn`) inside Brevi.

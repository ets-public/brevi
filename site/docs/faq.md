# FAQ
<!-- description: Short answers about Brevi: what it is, how it compares with Codon, Nuitka, Cython and PyPy, what it costs, and whether to use it. -->

## What is Brevi, in one sentence?

A compiler, written in Rust, that turns the plain Python an AI agent writes into a fast, memory-safe native binary with exactly CPython's output, and reports problems as one-line JSON an agent can act on.

## Why "AI-first"? Does it work for people too?

It works for anyone with a Python file. "AI-first" means the design is judged by what helps a model reach a working program cheaply: no new syntax to learn, short errors, a server that keeps the program so a fix costs only the changed lines, and a release test that runs real models. People get the same speed and the same short errors.

## Do I have to learn a new language?

No. Write Python. Brevi also has its own language for `.bv` files, but measurements showed models write plain Python more cheaply, so the agent tools ask for Python.

## How is this different from Codon, Nuitka, Cython or PyPy?

- **Codon** compiles a Python-like language and is very fast, but changes Python's behaviour (64-bit integers that wrap, different printing) and refuses much real code. On 591 agent programs it built 206 and got 36 of those wrong. Brevi built all 591, all matching CPython.
- **Nuitka** matches CPython's behaviour, but keeps CPython's object model, so it runs at about CPython's speed. Brevi is 1.7x to 40x faster on the same programs.
- **Cython** speeds up code you annotate with C types. Brevi needs no annotations.
- **PyPy** is a faster interpreter with a JIT. Brevi produces a standalone native binary, checks programs before they run, and gives errors built for agents.

[Benchmarks](/docs/benchmarks#brevi-codon-and-nuitka) has the full comparison.

## Will my program give the same answer?

That is the core promise: a program Brevi builds prints exactly what CPython prints, with the same exit status, or Brevi refuses to build it. It is checked on every change against hundreds of saved programs, behaviour traps and random programs. If you find a difference, it is a bug; please report it.

## What happens when Brevi cannot build my file?

`brevi run` runs it with `python3` and prints one line saying which line stopped Brevi. `brevi build`, `brevi check` and `brevi run --native` print the errors instead. See [the CPython fallback](/docs/usage#the-cpython-fallback).

## Can I use numpy, pandas or other packages?

Yes. Library calls run in CPython inside the native program, and the rest stays native. The machine running the program needs Python's shared library, or build with `--standalone` (Linux) to carry CPython in the binary. See [Libraries](/docs/compatibility#libraries-numpy-pandas).

## Why does it need Rust installed?

Brevi writes Rust and calls `rustc` to make the binary. The install script installs Rust if it is missing. The hosted service at brevi.run will remove that need.

## How fast are builds?

Well under a second for a new small program (about 0.15 to 0.3 s), a few seconds with `--release`. The very first build after installing also builds Brevi's runtime once, about 5 to 15 seconds. Builds are cached, so running the same program again skips `rustc`.

## Is it safe to run code an agent wrote?

Brevi programs are memory-safe, and integers never silently overflow. A program can declare what it touches (`allow read, write`, and `allow native` for crates and Python libraries), and `--sandbox` refuses anything the caller did not grant (`--allow`, `--allow-py`) on every build path, whatever the program's own allow lines say. A sandboxed program never falls back to CPython (from v0.24.1). Running untrusted code in a container or VM as well is still wise. See [Sandbox](/docs/usage#sandbox).

## Does it cost more tokens than Python?

No. In the release test for v0.25.0, over two runs, Brevi cost 1.09x (Sonnet) and 1.01x (Haiku) of the same models running Python with CPython, with every task passing, no check failing because of Brevi and 95 of 96 final programs built natively; two more runs just before its last fix gave 1.04x and 1.05x. v0.24.6 gave 1.03x and 1.02x, v0.24.5 gave 1.06x and 1.01x. For v0.24.4, over five runs, it cost 1.06x and 1.03x, with every task passing and 237 of 240 final programs built natively. Earlier releases gave 1.07x and 1.00x (v0.24.0) and 0.97x and 1.01x before that. The test runs again every week before the release. On heavy tasks, where the model would otherwise hand-optimise for CPython, it costs about half.

## Which platforms?

Linux, macOS and Windows on x86-64 and ARM64 (Windows x86-64 only). Library calls work on Linux and macOS, and `--standalone` builds are Linux only for now. See [Platforms](/docs/compatibility#platforms).

## Is it free? What is brevi.run?

The compiler is open source under Apache-2.0 and will stay that way. brevi.run is a planned hosted service, compile, run and test over HTTPS for agents that cannot install a toolchain, which may be paid later.

## Can I install it with pip, npm or Homebrew?

Not yet. The packaging is ready and the names are reserved, but nothing is published. Use the GitHub release for now; see [Install](/docs/install).

## Who makes Brevi?

Brevi is developed largely by AI coding agents (Claude) working under the direction of its maintainer, with every change tested against the benchmarks and regression suites before it is merged.

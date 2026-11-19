# FAQ
<!-- description: Short answers about Brevi: what it is, how it compares with Codon, Nuitka, Cython and PyPy, what it costs, and whether to use it. -->

## What is Brevi, in one sentence?

A compiler, written in Rust, that turns the plain Python an AI agent writes into a fast, memory-safe native binary with exactly CPython's output, and reports problems as one-line JSON an agent can act on.

## Why "AI-first"? Does it work for people too?

It works for anyone with a Python file. "AI-first" means the design is judged by what helps a model reach a working program cheaply: no new syntax to learn, short errors, a server that keeps the program so a fix costs only the changed lines, and a release test that runs real models. People get the same speed and the same short errors.

## Do I have to learn a new language?

No. Write Python. Brevi also has its own language for `.bv` files, but measurements showed models write plain Python more cheaply, so the agent tools ask for Python.

## How is this different from Codon, Nuitka, Cython or PyPy?

- **Codon** compiles a Python-like language and is very fast, but changes Python's behaviour (64-bit integers that wrap, different printing) and refuses much real code. On 591 of our saved agent programs it built 206 and got 36 of those wrong; Brevi built all 591, all matching CPython (they are our own regression set, so this favours Brevi). Codon never promised CPython's exact behaviour, and it is faster than Brevi on numeric code.
- **Nuitka** matches CPython's behaviour, but keeps CPython's object model, so it runs at about CPython's speed. Brevi's default typed build is 1.6x to 37.9x faster than CPython on our 12 compute benchmarks (median 9.3x), though only a small share of CPython's own test programs take that build; a program on the dynamic build runs at about CPython's speed (the dynamic build's median is 0.85x). Nuitka runs far more of Python than Brevi does today.
- **Cython** speeds up code you annotate with C types. Brevi needs no annotations.
- **PyPy** is a faster interpreter with a JIT. Brevi produces a standalone native binary, checks programs before they run, and gives errors built for agents.

[Benchmarks](/docs/benchmarks#brevi-codon-and-nuitka) has the full comparison.

## Will my program give the same answer?

That is the aim, and usually yes, but not always yet. Brevi tries to refuse at build time anything it cannot match, and every change is checked against hundreds of saved programs and behaviour traps. On CPython 3.13's own tests, 97.3% of the scored suite, with 83 string-as-code tests excluded match (1,043 of 1,072, at v0.28.2), 26 are refused at build time, and none build and answer differently or crash ([In numbers](/docs/compatibility#in-numbers)). For a program that matters, run `brevi verify x.py --input tests/`: it runs the native build and CPython on the same inputs and certifies them only if they agree. If you find a difference, it is a bug, so please report it.

## What happens when Brevi cannot build my file?

`brevi run` runs it with `python3` and prints one line saying which line stopped Brevi. `brevi build`, `brevi check` and `brevi run --native` print the errors instead. See [the CPython fallback](/docs/usage#the-cpython-fallback).

## Can I use numpy, pandas or other packages?

Yes. Most numpy code (arrays, broadcasting, indexing, reductions, printing) builds with Brevi's own numpy and needs no Python at run time; see [numpy](/docs/compatibility#numpy). From v0.26.0 so do requests, httpx, pydantic, the `anthropic` and `openai` SDKs, `mcp`, jinja2, polars, sqlite3, duckdb, tiktoken, tokenizers, safetensors and onnxruntime (see [What works](/docs/compatibility)). Other library calls, and numpy beyond that, run in CPython inside the native program, and the rest stays native. The machine running the program needs Python's shared library, or build with `--standalone` (Linux) to carry CPython in the binary. See [Libraries](/docs/compatibility#libraries-numpy-pandas).

## Why does it need Rust installed?

Brevi writes Rust and calls `rustc` to make the binary. The install script installs Rust if it is missing. The hosted service at brevi.run will remove that need.

## How fast are builds?

About 0.25 s for a tiny new program and a median of about 1.2 s for a typical agent program with no cache; about 1.5 s or more with `--release`. The very first build after installing also builds Brevi's runtime once, about 5 to 15 seconds. Builds are cached, so running the same program again skips `rustc`.

## Is it safe to run code an agent wrote?

Code you trust, yes; code that may be hostile, not on Brevi's sandbox alone. **No external security audit yet**: we have not had an external audit; we are seeking funding for one. Until then, do not run hostile code under the sandbox.

**The sandbox is a policy layer, not an isolation boundary.** Under `--sandbox` (the default in the MCP tools and the SDKs, except `verify`, which runs CPython) a program may touch only what the caller grants (`--allow`, `--allow-net`, `--allow-read`, `--allow-write`, `--allow-py`), checked before every build path and, for folders and hosts, again when the program runs, and its own `allow` lines grant nothing; a sandboxed program never falls back to CPython. That holds careless or overreaching agent code. But Brevi's own checks and runtime enforce it, with no container or VM around the program, so **run untrusted code in a container or VM** and use the sandbox as a second layer inside it. **`--allow-py` and `--allow native` are full trust**: grant them only to code you would run without a sandbox. Brevi builds programs as Rust and integers never silently overflow, which rules out some classes of bug, not the need for isolation. See [Sandbox](/docs/usage#sandbox).

## Does it cost more tokens than Python?

About the same. In the v0.28.0 release test (24 tasks, two runs), Brevi cost 1.08x (Sonnet) and 1.00x (Haiku) of what the same models cost running Python with CPython. Every task passed, and 95 of 96 final programs built natively. Recent releases have stayed between 0.97x and 1.09x for Sonnet, mostly a few percent above 1.00x, so Brevi does not save model cost on ordinary tasks. In this test, when Brevi cannot build a file it runs with CPython, as `brevi run` does, so the model does not see Brevi's build errors. On four heavy tasks, telling the model (on the Brevi side only) that its code is compiled and should be written plainly brought the cost to about half; CPython has not yet been run with the same line, so that does not yet show what Brevi itself saves. [Benchmarks](/docs/benchmarks#the-release-test) has every run.

## Which platforms?

Linux, macOS and Windows on x86-64 and ARM64 (Windows ARM64 from v0.28.1). Library calls through CPython work on Windows too from v0.27.0 (the Python on PATH must have its DLL); `--standalone` builds work on Linux and Windows from v0.28.1, not yet on macOS. See [Platforms](/docs/compatibility#platforms).

## Is it free? What is brevi.run?

The compiler is open source under Apache-2.0 and will stay that way. brevi.run is a planned hosted service, compile, run and test over HTTPS for agents that cannot install a toolchain, which may be paid later.

## Can I install it with pip, npm or Homebrew?

pip and npm, yes: `pip install brevi` or `npm install -g brevi` (or `uvx --python 3.13 brevi run --verify main.py`, `uvx brevi mcp` or `npx brevi mcp` without installing), and the crate is on crates.io. Homebrew not yet. See [Install](/docs/install).

## Who makes Brevi?

Brevi is developed largely by AI coding agents (Claude) working under the direction of its maintainer, with every change tested against the benchmarks and regression suites before it is merged.

## How do I get help or contact you?

Email [support@brevi-lang.org](mailto:support@brevi-lang.org) for questions, licensing or anything you would rather not post in public. Bugs and feature requests are best as a GitHub issue, and security reports go through [SECURITY.md](repo:SECURITY.md).

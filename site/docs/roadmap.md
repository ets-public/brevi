# Roadmap
<!-- description: What is next for Brevi: the public preview, package managers, brevi.run, and the compiler work in progress. -->

Brevi's direction stays the same: **AI-first**, with speed and safety as the reasons an agent should run its Python through Brevi. Token savings are added step by step and each one is measured in the release test before it counts. This page lists what is planned, not promised dates.

## Now

- **Public preview on GitHub.** The source, the releases and these docs, under Apache-2.0.
- **Sandbox on every path** (v0.24.1). The `allow` check runs before every build path, Python libraries need `allow native`, and a sandboxed program never falls back to CPython. From v0.24.6 only the caller grants capabilities under `--sandbox` (`--allow`, `--allow-py`, the MCP `allow` and `allow_py` arguments); the program's own allow lines grant nothing.
- **Library programs off the typed path** (v0.24.6). A program the typed build rejects that imports a library Brevi lacks (requests, pydantic, yaml) takes the dynamic build, and only those imports run in CPython.
- **The standard library, natively.** Standard-library calls no longer need CPython. What the typed build lacks runs in the native dynamic build (since v0.24.3), and common modules move onto the typed fast path one at a time; the [changelog](/docs/changelog) lists each one. Next: `csv.writer` on other files, `datetime.strptime` on the typed path, `IntEnum` and `Flag`, then `json` indent and `dataclasses.asdict` on the typed path. The [table](/docs/compatibility#what-runs-native) shows where each function runs.
- **More programs on the fast path.** On a corpus of 813 saved agent programs, 567 build typed in v0.25.0 (565 in v0.24.5, 550 in v0.24.4, 496 in v0.24.3), and v0.24.5 fixes the typed-build wrong answers the v0.24.4 release test found (`trains`, `poker`). Next: multi-file programs where one module sends the whole program to CPython.
- **numpy, natively.** Python files that use numpy build without CPython: Brevi's own numpy (float64, int64, float32, int32, bool and string arrays, broadcasting, ufuncs, reductions in numpy's summation order, indexing, numpy's printing) checked against real numpy. What it does not have yet goes to CPython's numpy as before. `np.random` draws numpy's own numbers for the same seed (all the common distributions), and matrix products, `linalg` (`det`, `solve`, `inv`, `norm`, `lstsq`), `polyfit`, `histogram`, `cov` and `corrcoef` match numpy to the last digit. All of numpy's common random distributions are native too. Next: float32 matrix products, `np.strings` functions and numpy's multi-threaded LU for large matrices.
- **Tokenizers, natively.** `tiktoken` builds natively from the next release, with every OpenAI encoding built into the binary, so token counting and chunking need no CPython or network. Hugging Face `tokenizers` builds natively too (BPE and WordPiece tokenizers from the Hub, built into the binary). Next: Unigram models (T5, XLM-R).
- **Weekly releases.** One release a week, on Mondays, after that week's release test passes. Changes land on `main` under "Unreleased" in the [changelog](/docs/changelog) until then.
- **Cheaper heavy tasks.** Tell agents their Python is compiled so they write the plain solution, and make plain idioms (generators, string indexing) build typed and fast.

## Next

- **Homebrew.** `brew install ets-public/brevi/brevi`; the formula is ready. (`pip install brevi`, `npx brevi` and the MCP registry are live, and `cargo install brevi` follows with the next release.)
- **brevi.run.** The compiler as a hosted API for agents: check, run and test over HTTPS, sandboxed, with cached builds. Then a playground in the browser.
- **More of Python natively.** `match` statements and more standard modules in dynamic builds. (`async` / `await`, `asyncio`, async generators, `ExceptionGroup` and `except*` build natively from the next release.)

## Later

- **numpy-style arrays in `.bv`.** Brevi's own language gets an array type with what native numpy has for Python files, on the same engine.
- **Windows parity.** Library calls and `--standalone` on Windows; `--standalone` on macOS.
- **Faster cold builds**, for agents that build many one-off programs.
- **Larger and fresher benchmarks** on new model versions, with results published here (the release test already runs weekly).

## Always

- Every program that once passed must keep passing: the full set of saved solutions is rerun on every change and on every new Rust and Python release.
- Brevi matches CPython or refuses. A silent difference is treated as the worst kind of bug.

Changes ship in releases listed in the [changelog](/docs/changelog).

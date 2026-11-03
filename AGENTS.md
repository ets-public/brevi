# Working on the Brevi compiler

Brevi is a language for AI models that compiles to Rust. The guide models read is BREVI.md; the compiler is in src/ (lexer, parser, infer, emit, prelude).

- Build: `cargo build --release`. Unit tests: `cargo test --release`.
- Language tests: `BREVI=./target/release/brevi tests/check.sh` (every file in tests/run and tests/errors must match its .expected).
- Compatibility: `bench/verify.sh OLD_BREVI NEW_BREVI` re-runs every past bench solution. No change may alter the output of a program that worked before.
- Before merging: `sh tools/ci_local.sh` runs the PR checks locally (GitHub Actions minutes are limited; note the result in the PR).
- Toolchain compatibility: `python3 tools/compat.py check` rebuilds every solution in compat/baseline.txt with the rustc and python3 on PATH (`RUSTUP_TOOLCHAIN=nightly` to try another). Never delete baseline lines to make it pass; run `tools/compat.py baseline` only to add newly passing ones.
- Benchmarks: bench/run.sh runs one task; bench/analyze.py prints tokens per finished task. Scores are exact token counts, so never print the key in BENCH_ANTHROPIC_KEY.
- Keep BREVI.md short: every line costs tokens in every session that reads it. Library use lives in LIBRARIES.md (`brevi guide Libraries`) and details in REFERENCE.md (`brevi guide Strings`, `brevi guide partition`), which sessions read only when they need them. `tools/guide_budget.py` fails CI when BREVI.md passes 3,600 tokens.
- New syntax goes into `grammar/brevi.lark` too (then run `python3 tools/lark_to_gbnf.py`). CI parses every `tests/run` program with the grammar, so a test using the new syntax fails until the grammar has it.
- Libraries: src/modules.rs joins `use "x.bv"` files into one program and maps error lines back to files; src/ffi.rs handles `use CRATE "VER"`, `rust fn`, `py fn` and `rust:`/`py:` blocks. Programs with crates build through cargo with a shared target dir ($BREVI_CACHE or ~/.cache/brevi); others still call rustc directly. tests/run/crate_use.bv needs network on its first run, py_fn.bv needs python3.
- Caches: src/runcache.rs replays an unchanged `check`/`build`/`run` (the build runs as a child with BREVI_PHASE=1); under it, `compile()` in src/main.rs caches rustc results by generated code. Anything new a build reads (a file, an env var) must be in runcache's key, or a stale result is replayed. `--no-cache` turns both off; `BREVI_TIME=1` prints stage timings.
- Keep the GitHub info current with every user-visible change: add a line under "Unreleased" in CHANGELOG.md (each release's section becomes its release notes), and update README.md's status, numbers and feature list in the same PR. The site's version and guide size are filled in by site/build.sh; its benchmark table and feature text are edited by hand in site/index.html.

To write Brevi programs rather than work on the compiler, use skills/brevi/SKILL.md.

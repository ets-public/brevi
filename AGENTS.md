# Working on the Brevi compiler

Brevi is a language for AI models that compiles to Rust. The guide models read is BREVI.md; the compiler is in src/ (lexer, parser, infer, emit, prelude).

- Build: `cargo build --release`. Unit tests: `cargo test --release`.
- Language tests: `BREVI=./target/release/brevi tests/check.sh` (every file in tests/run and tests/errors must match its .expected).
- Compatibility: `bench/verify.sh OLD_BREVI NEW_BREVI` re-runs every past bench solution. No change may alter the output of a program that worked before.
- Toolchain compatibility: `python3 tools/compat.py check` rebuilds every solution in compat/baseline.txt with the rustc and python3 on PATH (`RUSTUP_TOOLCHAIN=nightly` to try another). Never delete baseline lines to make it pass; run `tools/compat.py baseline` only to add newly passing ones.
- Benchmarks: bench/run.sh runs one task; bench/analyze.py prints tokens per finished task. Scores are exact token counts, so never print the key in BENCH_ANTHROPIC_KEY.
- Keep BREVI.md short: every line costs tokens in every session that reads it. Library use lives in LIBRARIES.md (`brevi guide Libraries`) and details in REFERENCE.md (`brevi guide Strings`, `brevi guide partition`), which sessions read only when they need them. `tools/guide_budget.py` fails CI when BREVI.md passes 3,600 tokens.
- Libraries: src/modules.rs joins `use "x.bv"` files into one program and maps error lines back to files; src/ffi.rs handles `use CRATE "VER"`, `rust fn`, `py fn` and `rust:`/`py:` blocks. Programs with crates build through cargo with a shared target dir ($BREVI_CACHE or ~/.cache/brevi); others still call rustc directly. tests/run/crate_use.bv needs network on its first run, py_fn.bv needs python3.

To write Brevi programs rather than work on the compiler, use skills/brevi/SKILL.md.

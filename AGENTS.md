# Working on the Brevi compiler

Brevi is a language for AI models that compiles to Rust. The guide models read is BREVI.md; the compiler is in src/ (lexer, parser, infer, emit, prelude).

- Build: `cargo build --release`. Unit tests: `cargo test --release`.
- Language tests: `BREVI=./target/release/brevi tests/check.sh` (every file in tests/run and tests/errors must match its .expected).
- Compatibility: `bench/verify.sh OLD_BREVI NEW_BREVI` re-runs every past bench solution. No change may alter the output of a program that worked before.
- Toolchain compatibility: `python3 tools/compat.py check` rebuilds every solution in compat/baseline.txt with the rustc and python3 on PATH (`RUSTUP_TOOLCHAIN=nightly` to try another). Never delete baseline lines to make it pass; run `tools/compat.py baseline` only to add newly passing ones.
- Benchmarks: bench/run.sh runs one task; bench/analyze.py prints tokens per finished task. Scores are exact token counts, so never print the key in BENCH_ANTHROPIC_KEY.
- Keep BREVI.md short: every line costs tokens in every session that reads it.

To write Brevi programs rather than work on the compiler, use skills/brevi/SKILL.md.

# Axon

A programming language for AI models to write. Axon compiles to Rust, so programs get Rust's speed and safety, but the model never writes lifetimes, borrows or semicolons, and every compiler error comes back as one JSON line with a suggested fix.

The guide a model reads before writing Axon is [AXON.md](AXON.md), about 1,200 tokens.

## Status: v0.3

Works today:
- Parser for the full core syntax in AXON.md
- Front-end checks with short JSON errors: unknown names and methods with "did you mean" hints, a missing `?` on a call that can fail, wrong argument counts
- Emits Rust and builds it with `rustc`. rustc errors are mapped back to the Axon line and rewritten in Axon's type names
- `run`, `test`, `check`, `build`, `emit`
- Runtime prelude with strings, lists, sorted maps, files, stdin, JSON, CSV, dates and big integers
- `?` works inside lambdas, `!` is inferred, `.or(default)` handles a failure in place
- Errors list the valid methods or fields; runtime errors are one JSON line with the Axon line
- Benchmark in `bench/`: 22 tasks, results for Claude Sonnet and Haiku in Axon, Python and Rust

Not done yet:
- Ownership inference. v0 clones every value it reads, so code is always correct but slower than hand-written Rust
- Axon's own type inference. Type errors currently come from rustc, translated
- Edit-by-name patches and compact output printing
- A priority queue, and try/catch-style recovery beyond `.or()`

## Build and try

```
cargo build --release
./target/release/axon run examples/shapes.ax
./target/release/axon test examples/shapes.ax
tests/check.sh            # regression tests: sample programs and error cases
```

Needs `rustc` on the PATH.

## Layout

| Path | What it is |
| --- | --- |
| `src/lexer.rs`, `src/parser.rs`, `src/ast.rs` | Source to syntax tree |
| `src/emit.rs` | Checks and Rust generation |
| `src/prelude.rs` | Runtime pasted into every generated program |
| `src/main.rs` | CLI, rustc invocation and error mapping |
| `examples/` | Sample programs |
| `tests/` | Expected outputs for programs and error cases |

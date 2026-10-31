# Brevi

A programming language for AI models to write. Brevi compiles to Rust, so programs get Rust's speed and safety, but the model never writes lifetimes, borrows or semicolons, and every compiler error comes back as one JSON line with a suggested fix.

The guide a model reads before writing Brevi is [BREVI.md](BREVI.md), about 2,700 tokens (estimated).

## Status: v0.9

Works today:
- Parser for the full core syntax in BREVI.md
- Front-end checks with short JSON errors: unknown names and methods with "did you mean" hints, wrong argument counts, names used outside their block
- Emits Rust and builds it with `rustc`. rustc errors are mapped back to the Brevi line and rewritten in Brevi's type names
- `run`, `test`, `check`, `build`, `emit`
- Runtime prelude with strings, lists, maps (hash lookups, listed in key order), files, stdin, JSON, CSV, dates, big integers, exact decimals and a min-heap
- JSON objects from map literals (`str({"id": 1, "name": n})`), `x.json()`, and Python-syntax regexes (`re_full`, `re_find`, `re_all`, `re_sub`, `re_split`)
- Python-style indented blocks (`if x:` and an indented body), with braces still accepted
- Empty lists, maps and heaps need no type: the compiler works it out from later use
- Short syntax: `x = 1` declares, `x: [str] = []` gives a type, `(a, b) = pair`, errors pass up without `?`, field defaults and shorthand constructors
- `.or(default)` and `match f(x) { Ok(v) => ... Err(m) => ... }` handle failures; runtime errors are one JSON line with the Brevi line
- Ownership inference: a variable's last read moves its value, keys, comparisons and indexes read in place, and loops read lists without copying them. If a move would break Rust's borrow rules, the compiler quietly builds the copying version instead
- Benchmarks in `bench/`: 8 bigger tasks (50 to 200 lines), 10 hard tasks, 8 held-out tasks written after the language was tuned, 4 edits to an existing program, and 4 speed tests, with results for Claude Sonnet and Haiku in Brevi, Python and Rust

Tokens per finished task set (18 tasks: code written plus errors read, average of 2 runs; Python from earlier runs of the same tasks):

| Model | Brevi v0.8 | Brevi v0.7 | Python | First try (Brevi v0.8) |
| --- | --- | --- | --- | --- |
| Sonnet | 4,309 | 4,363 | 4,021 | 36 of 36 |
| Haiku | 4,987 | 5,631 | 5,266 | 35 of 36 |

Brevi now beats Python on tokens with Haiku and is 7% above it with Sonnet.

On the 8 bigger tasks (v0.9, 2 runs each), finished code is close to Python's size, but retries still cost more:

| Model | Brevi finished code | Python finished code | Brevi with retries | Python with retries | First try (Brevi) |
| --- | --- | --- | --- | --- | --- |
| Sonnet | 7,786 | 7,418 | 11,121 | 7,450 | 14 of 16 |
| Haiku | 9,705 | 9,751 | 17,107 | 10,634 | 9 of 16 | Rust takes about 2.4 times as many tokens as Brevi on the hard tasks.

Speed (best of 3, seconds):

| Program | Python | Brevi | Rust |
| --- | --- | --- | --- |
| Word counts | 0.35 | 0.18 | 0.09 |
| Dijkstra | 2.90 | 0.47 | 0.30 |
| Prime sieve | 1.33 | 0.05 | 0.02 |
| Group records | 0.76 | 0.31 | 0.12 |

Not done yet:
- Brevi's own type inference. Type errors currently come from rustc, translated
- Edit-by-name patches and compact output printing

## Build and try

```
cargo build --release
./target/release/brevi run examples/shapes.bv
./target/release/brevi test examples/shapes.bv
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

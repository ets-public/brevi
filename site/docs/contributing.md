# Contributing and license
<!-- description: How to build Brevi from source, run its tests, find your way around the code, report bugs, and the Apache-2.0 license. -->

Brevi is open source under the Apache License 2.0. Bug reports, failing programs and fixes are all welcome.

## Build and test

```
git clone __GITHUB__
cd brevi
cargo build --release
tests/check.sh            # sample programs and error cases against their expected output
sh tools/ci_local.sh      # the same checks a pull request runs, locally
```

You need Rust stable and `python3` (the tests compare Brevi's output with CPython's).

| Check | What it guards |
| --- | --- |
| `tests/check.sh` | Every program in `tests/run`, `tests/errors`, `tests/typed`, `tests/dyn`, `tests/fallback` and `tests/standalone` matches its expected output. |
| `bench/verify.sh OLD NEW` | Every saved benchmark solution prints the same with a new compiler. |
| `tools/compat.py check` | Every solution in `compat/baseline.txt` keeps building on new Rust and Python releases. Never remove a line to make it pass. |
| `tools/parity_probe.py` | Short Python probes run with CPython and Brevi; lists silent differences and refusals. |

## Where things are

| Path | What it is |
| --- | --- |
| `src/main.rs` | The command line, the build driver, `rustc` and error mapping |
| `src/lexer.rs`, `src/parser.rs`, `src/ast.rs` | Source to syntax tree, for both Python and Brevi |
| `src/infer.rs`, `src/emit.rs`, `src/moves.rs` | Type inference and checks, Rust generation, ownership |
| `src/dynpy/` | The dynamic build and its runtime |
| `src/pybridge.rs`, `src/pyrt.rs` | Library calls through CPython |
| `src/prelude.rs` | The runtime of typed builds |
| `src/mcp.rs` | The MCP server |
| `runtime/brevi_tk` | The tkinter runtime, headless and windowed |
| `bench/` | Benchmarks, logs and saved solutions |
| `docs/ARCHITECTURE.md` | How it all fits together (also on this site as [Architecture](/docs/architecture)) |
| `site/` | This website |

`AGENTS.md` in the repository has the working notes for coding agents changing the compiler.

## Reporting a bug

The most useful report is a short Python file and its input where `brevi run --native` prints something different from `python3`, or refuses a program you think it should build. Open an issue<!--public--> on [GitHub](__GITHUB__/issues)<!--/public--> with the file, the input, both outputs and `brevi --version`. Wrong output is the highest priority.

Please report security problems privately, as described in the repository's `SECURITY.md`, not in a public issue.

## Pull requests

- Keep each change focused, with a test in `tests/` that fails before it and passes after.
- Run `sh tools/ci_local.sh` before opening the pull request.
- Add a line under "Unreleased" in `CHANGELOG.md` describing the change in plain words.

## License

Brevi is licensed under the [Apache License, Version 2.0](repo:LICENSE). You may use, modify and distribute it, including commercially, under its terms. Contributions are accepted under the same license. The fonts on this site are IBM Plex, under the [SIL Open Font License](/OFL.txt).

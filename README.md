<img src="site/icons/favicon.svg" alt="Brevi logo" width="72" height="72">

# Brevi

**The fast, safe target for AI-written Python.** Brevi is a compiler built for AI coding agents. The agent writes the plain Python it already writes best; Brevi builds it through Rust into a native binary that prints exactly what CPython prints and runs 1.7x to 40x faster with the default build, and it answers anything it cannot build with one short JSON line the agent can act on. The model never writes lifetimes, borrows or semicolons, and needs no guide to start.

Docs: **[brevi-lang.org/docs](https://brevi-lang.org/docs/)** · Releases: [GitHub](https://github.com/ets-public/brevi/releases/latest) · Changes: [CHANGELOG.md](CHANGELOG.md)

**Public preview.** Brevi works today and is tested against CPython on every change, but it is young: expect rough edges, and expect the CLI and Brevi's own syntax to change between releases. Bug reports are very welcome ([CONTRIBUTING.md](CONTRIBUTING.md)).

## Install

Linux and macOS:

```
curl -fsSL https://raw.githubusercontent.com/ets-public/brevi/main/install.sh | sh
```

The script installs the Rust toolchain if it is missing (Brevi compiles through `rustc`), then the prebuilt `brevi` for your machine from the [latest release](https://github.com/ets-public/brevi/releases/latest) into `~/.cargo/bin`, verified against the release's `SHA256SUMS`, or builds it with `cargo install` when no prebuilt one fits.

Windows: install Rust with [rustup](https://rustup.rs), then unzip `brevi-x86_64-pc-windows-msvc.zip` from the [latest release](https://github.com/ets-public/brevi/releases/latest) and put `brevi.exe` on your PATH. Python files that import libraries such as numpy run with CPython there for now.

With a package manager (Rust is still needed, from [rustup](https://rustup.rs)):

```
pip install brevi          # or: uvx brevi mcp
npm install -g brevi       # or: npx brevi mcp
```

`cargo install brevi` works from the next release. Docker, building from source and the release archives: [Install](https://brevi-lang.org/docs/install).

## Quick start

```
$ printf 'print(sum(i * i for i in range(10)))\n' > main.py
$ brevi run main.py                     # build natively (cached), then run
285
$ brevi check main.py                   # build without running: errors only
$ brevi build main.py -o app --release  # an optimised binary to keep or ship
$ brevi verify main.py --input tests/   # run it and python3 on each input: a certificate if they agree
```

A sure mistake is reported before anything runs, as one line with a fix:

```
$ brevi check broken.py
{"line":8,"msg":"name 'totl' is not defined","fix":"did you mean total?"}
```

Mistakes that Python itself only finds when a line runs raise there, as in CPython. When Brevi cannot build a file natively yet, or it has a sure mistake, `brevi run` runs it with `python3` and prints one JSON line naming the line and function that stopped it; `brevi build`, `brevi check` and `brevi run --native` never fall back. Files can import their sibling modules and the standard library, which Brevi builds natively itself ([what runs native](https://brevi-lang.org/docs/compatibility#what-runs-native)), and installed packages (numpy, pandas, sqlite3, ...), whose calls run in CPython inside the native program; `brevi build x.py --standalone` packs CPython and those packages into the binary (Linux). More in [Using Brevi](https://brevi-lang.org/docs/usage).

## For agents

- **MCP server:** `brevi mcp` serves `check`, `run`, `test`, `verify`, `patch`, `edit`, `show` and `guide` over stdio. Add it as the command `brevi` with the argument `mcp` (Claude Code: `claude mcp add brevi -- brevi mcp`). The agent sends plain Python as `code` once; the server keeps it, and `patch` (whole `def`s and classes by name) or `edit` (a range of lines) change only what they are sent and check again, so a retry costs only the changed lines. `run` with `cases` builds once and prints `ok` or the first differing line per case; `verify` runs the native build and CPython on the same inputs and returns a certificate when they agree. The tools sandbox by default (`verify`, which runs CPython, needs `sandbox: false`). Claude Desktop setup (it needs the full path to `brevi`), time and memory limits and troubleshooting: [For AI agents](https://brevi-lang.org/docs/agents#mcp-server).
- **Skill and plugin:** [skills/brevi/SKILL.md](skills/brevi/SKILL.md) is an Agent Skills package (setup, then write plain Python and fix only the lines an error names). In Claude Code, `/plugin marketplace add ets-public/brevi` and `/plugin install brevi@brevi` add the skill and the MCP server together.
- **llms.txt:** [brevi-lang.org/llms.txt](https://brevi-lang.org/llms.txt), [llms-full.txt](https://brevi-lang.org/llms-full.txt), and every docs page as Markdown (add `.md` to its address).
- **Editor rules** for Cursor, Copilot, Windsurf and Cline in [skills/rules/](skills/rules/), and tested examples for LangChain, the OpenAI Agents SDK, CrewAI and the Vercel AI SDK in [integrations/](integrations/README.md).
- **Agent platform templates** in [templates/](templates/README.md): run the Python an agent writes on E2B, Daytona, Modal or Cloudflare Sandbox (or Docker), with only the capabilities and limits your code grants, from one image and one `run_python` tool.
- **Prompting:** ask for the plain, direct solution and say it will be compiled. Models that expect CPython hand-optimise, which costs tokens and is often slower under Brevi. Details in [For AI agents](https://brevi-lang.org/docs/agents).

## How it works

Brevi tries the fastest exact path first. The **typed build** works out a fixed type for every value and writes plain Rust; most programs build this way. A program whose values change type, or that the typed build cannot match exactly, gets the **dynamic build**: still native Rust, over values that behave like CPython's objects. Most numpy code builds with Brevi's own numpy, matching numpy's output byte for byte (on `main`, next release). Other **library calls** (pandas, requests, ...) run in CPython loaded into the native program. Only when none of these can build a file does `brevi run` fall back to `python3`. Every build off the typed path says which path ran and why in one JSON line (`{"path":"dynamic","line":3,"fn":"show","why":...}`), so an agent knows which function to rewrite. Integers grow past 64 bits as in Python and never silently wrap. Under `--sandbox`, and in the MCP tools, which sandbox by default, a program gets no files, native code or libraries unless the caller grants them (`--allow read,write`, `--allow-py requests`); the code cannot grant itself anything, a refusal is final, and a sandboxed program never falls back to `python3`. `--limit cpu=2,mem=512,time=10,output=64` (MCP `limits`) stops a program at the first CPU, memory, wall-clock, output or file-size limit it reaches ([threat model](docs/THREAT_MODEL.md)). Diagrams and details: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

Brevi also has its own shorter Python-like language for `.bv` files, checked fully before it runs: [BREVI.md](BREVI.md) (3,508 tokens, served by `brevi guide`), [REFERENCE.md](REFERENCE.md) and [LIBRARIES.md](LIBRARIES.md) (Rust crates and Python from Brevi). Agents finish tasks more cheaply in plain Python, so the agent tools ask for Python.

## Status: v0.25.0

- **Same answers as CPython.** On unchanged Python files, Brevi builds all 591 saved agent programs and all 18 large programs (chess, a text adventure, an interpreter), and every one matches CPython's output and exit status. Codon builds 206 of the 591 and gets 36 of those wrong; Nuitka matches all of them but runs at about CPython's speed ([bench/compare](bench/compare/README.md)).
- **Measured against CPython's own tests.** `tools/conformance.py` runs the tests of CPython 3.13's `test_grammar`, `test_scope`, `test_int`, `test_list` and `test_dict` through Brevi: 180 of 311 scored tests (57.9%) print exactly what CPython prints, 67 are refused at build time, and 45 build but differ (18 in output, 27 in exit status; 79 before v0.25.0). Most of these are corner cases CPython's tests probe on purpose, and the next releases work that number down ([report](bench/conformance/report.md)). For your own program, `brevi verify x.py --input tests/` runs the native build and CPython on the same inputs and prints a certificate (file and input hashes, path, speedup) when stdout, exit code and stderr agree, or the first line that differs (on `main`, next release).
- **Faster.** The default build beats CPython on all 12 speed programs (1.7x to 40x); `--release` is level with Codon where Codon builds. `brevi check` answers in well under a second (median 144 ms for a new file), an unchanged program is re-checked from cache in about 10 ms, and standard-library calls (`math`, `itertools`, `statistics`, `datetime`, `enum`, `io.StringIO`, path text from `os` and `pathlib`, ...) run natively, not in CPython: every final program from the v0.24.3 release tests now builds natively, and 567 of 813 saved agent programs take the fast typed build.
- **No more expensive for agents.** In the release test, Claude Sonnet and Haiku solve 24 tasks in plain Python, once checked by CPython and once by Brevi, and the whole API bill is compared. The bar: every task passes, Brevi's cost is within 10% of CPython's, and at least 90% of final programs build natively. v0.24.0 met it over two runs (1.07x for Sonnet, 1.00x for Haiku, 94% and 98% of final programs native), v0.24.4 met it over five runs (1.06x and 1.03x), v0.24.5 over two runs (1.06x and 1.01x), v0.24.6 over two runs (1.03x and 1.02x), and v0.25.0 meets it over two runs: 1.09x and 1.01x, every task passed, no check failed because of Brevi, 47 of 48 and 48 of 48 final programs native (two runs just before its last fix: 1.04x and 1.05x, 96 of 96 native). On four heavy tasks where a model writing for CPython must hand-optimise, telling it the code is compiled brought the cost to 0.58x (Sonnet) and 0.53x (Haiku).
- **Known gaps:** `async`, `match` statements and `eval` are not built natively yet, classes built on a library's classes run in CPython as a whole, library calls and `--standalone` are not on Windows yet. See [What works, and limits](https://brevi-lang.org/docs/compatibility) and the [roadmap](https://brevi-lang.org/docs/roadmap).

Releases come out once a week, on Mondays, after the release test passes (an important fix may ship sooner); until then changes collect under "Unreleased" in [CHANGELOG.md](CHANGELOG.md).

Method and logs: [bench/README.md](bench/README.md) and [Benchmarks](https://brevi-lang.org/docs/benchmarks). Results from v0.8 to v0.23, when Brevi's own language was the main target, are in [bench/HISTORY.md](bench/HISTORY.md).

## Build from source

```
git clone https://github.com/ets-public/brevi
cd brevi
cargo build --release
./target/release/brevi run examples/top.py < examples/input.txt
sh tools/ci_local.sh      # the checks a pull request runs
```

Needs Rust stable and `python3`. Working on the compiler itself: [AGENTS.md](AGENTS.md).

## Layout

| Path | What it is |
| --- | --- |
| `docs/ARCHITECTURE.md` | How a source file becomes a native program, with diagrams |
| `src/lexer.rs`, `src/parser.rs`, `src/ast.rs` | Source to syntax tree |
| `src/emit.rs` | Checks and Rust generation |
| `src/prelude.rs` | Runtime pasted into every generated program |
| `src/main.rs` | CLI, rustc invocation and error mapping |
| `src/caps.rs` | `allow` lines and the `--sandbox` check |
| `examples/` | Sample programs |
| `tests/` | Expected outputs for programs and error cases |
| `tools/parity_probe.py`, `tools/parity/` | Short plain-Python probes run with CPython and Brevi; lists silent differences and refusals |
| `tools/conformance.py` | CPython's own test suite, cut into standalone scripts and built with Brevi; [latest result](bench/conformance/report.md) |
| `tools/compat.py`, `compat/baseline.txt` | Every bench solution that ever passed; must keep passing on new toolchains |
| `integrations/` | Examples for LangChain, OpenAI Agents SDK, CrewAI and Vercel AI SDK over `brevi mcp` |
| `templates/` | Agent sandboxes on E2B, Daytona, Modal, Cloudflare and Docker, running code under Brevi's sandbox and limits |
| `grammar/`, `tools/grammar_check.py`, `tools/lark_to_gbnf.py` | Grammar for constrained decoding, and its checks |
| `tools/upstream_watch.py` | Opens review issues for new Rust, Python and crate releases |
| `.github/` | Daily toolchain matrix, upstream watch, auto-fix, Dependabot (see "Staying current") |

## Staying current

Brevi must never fall behind Rust or Python, and code that once passed must never silently break.

- **Toolchains** (`.github/workflows/toolchains.yml`): builds Brevi and reruns `tests/check.sh` plus `tools/compat.py check`
  (all 1,900+ saved Brevi, Python and Rust solutions that once passed). Daily on Rust stable with the newest and the next
  Python pre-release; Mondays also on Rust beta and nightly, where nightly reports deprecation and future-incompatibility
  warnings in generated Rust and the newest edition, about 6 to 12 weeks before they reach stable. PRs test Rust stable
  only, skip drafts and docs-only changes, and rerun the old solutions only when the compiler or baseline changed.
  A toolchain already tested at the same commit is skipped. Failures open a `toolchain` issue. To save Actions minutes,
  run `sh tools/ci_local.sh` (the same checks) before merging.
- **Upstream releases** (`upstream.yml`, twice a week): one `upstream` issue per new Rust stable, Python release or
  newer crate pinned in tests/examples, listing the new language features and stabilized APIs with a review checklist.
- **Auto-fix** (`auto-fix.yml`): Claude Code picks up `toolchain` and `upstream` issues and opens a PR, using the
  `ANTHROPIC_API_KEY` repository secret; a human reviews every PR.
- **Dependabot**: Cargo dependencies and GitHub Actions versions, weekly.

After a change that makes more solutions pass, run `python3 tools/compat.py baseline` to lock them in.
Never remove a line from `compat/baseline.txt` to make a check pass.

## Contributing and security

See [CONTRIBUTING.md](CONTRIBUTING.md) for bug reports and pull requests, and [SECURITY.md](SECURITY.md) to report a vulnerability privately. For anything else, email [support@brevi-lang.org](mailto:support@brevi-lang.org).

## License

Apache License 2.0. See [LICENSE](LICENSE).

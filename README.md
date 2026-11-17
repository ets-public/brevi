<img src="site/icons/favicon.svg" alt="Brevi logo" width="72" height="72">

# Brevi

**The fast, safe target for AI-written Python.** Brevi is a compiler built for AI coding agents. The agent writes the plain Python it already writes best; Brevi builds it through Rust into a native binary that runs 1.6x to 37.9x faster than CPython on our 12 compute benchmarks with the default typed build (median 9.3x; a program that falls back to the dynamic build runs at about CPython's speed, median 0.85x: [numbers](docs/NUMBERS.md)), and aims to print exactly what CPython prints (at v0.28.1, 97.3% of the scored suite, with 83 string-as-code tests excluded (1,043 of 1,072 from CPython 3.13's own suite) match and none build and differ; 96.9% of 2,053 tests for the most used standard-library modules, 6 left out the same way. Counting only matches that print something to compare, 88.2% and 94.3%; 2.6% and 1.2% of the matches take the typed fast path, the rest the dynamic build), and it answers anything it cannot build with one short JSON line the agent can act on. The model never writes lifetimes, borrows or semicolons, and needs no guide to start.

Docs: **[brevi-lang.org/docs](https://brevi-lang.org/docs/)** · Releases: [GitHub](https://github.com/ets-public/brevi/releases/latest) · Changes: [CHANGELOG.md](CHANGELOG.md)

**Public preview.** Brevi works today and is tested against CPython on every change, but it is young: expect rough edges, and expect the CLI and Brevi's own syntax to change between releases. It is **not yet a drop-in `python3`**: on CPython's own tests some built programs still answer differently (see Status), `brevi run` quietly falls back to `python3` for files it cannot build (`--native` turns that off), and the sandbox is a policy layer, not an isolation boundary. Bug reports are very welcome ([CONTRIBUTING.md](CONTRIBUTING.md)).

**Security, plainly.** No external security audit yet: we have not had an external audit; we are seeking funding for one. Until then, do not run hostile code under the sandbox. The sandbox is a policy layer, not an isolation boundary: Brevi's own checks hold the program, with no container or VM around it. Run untrusted code in a container or VM, and use the sandbox as a second layer inside it. `--allow-py` and `--allow native` are full trust. Who should use Brevi today, and what a public launch needs first: [SECURITY.md](SECURITY.md#where-brevi-stands).

## Install

Linux and macOS:

```
curl -fsSL https://raw.githubusercontent.com/ets-public/brevi/main/install.sh | sh
```

No Rust install is needed on Linux x86-64 and macOS from the next release: with no `rustc` on the PATH, brevi fetches the Rust release it is tested with (and zig to link, if there is no C linker) on its first build, checked against SHA-256 sums compiled into brevi; with [uv](https://docs.astral.sh/uv/), `uvx --python 3.13 brevi run --verify main.py` is the whole install. Linux ARM64 and Windows follow as each passes CI ([the pinned toolchain](https://brevi-lang.org/docs/install#the-pinned-toolchain)).

The script installs the prebuilt `brevi` for your machine from the [latest release](https://github.com/ets-public/brevi/releases/latest) into `~/.cargo/bin`, verified against the release's `SHA256SUMS`, or builds it with `cargo install` when no prebuilt one fits, then fetches the pinned toolchain, or installs Rust with rustup where that is not verified yet.

Windows: install Rust with [rustup](https://rustup.rs), then unzip `brevi-x86_64-pc-windows-msvc.zip` (`brevi-aarch64-pc-windows-msvc.zip` on ARM64, from v0.28.1) from the [latest release](https://github.com/ets-public/brevi/releases/latest) and put `brevi.exe` on your PATH. From v0.27.0, Python files that import libraries build too: their library calls run in CPython through the bridge (the `python` on PATH needs its DLL).

With a package manager (Rust is still needed, from [rustup](https://rustup.rs)):

```
pip install brevi          # or: uvx brevi mcp
npm install -g brevi       # or: npx brevi mcp
```

`cargo install brevi` works from v0.26.0. Docker, building from source and the release archives: [Install](https://brevi-lang.org/docs/install).

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

Mistakes that Python itself only finds when a line runs raise there, as in CPython. When Brevi cannot build a file natively yet, or it has a sure mistake, `brevi run` runs it with `python3` and prints one JSON line naming the line and function that stopped it; `brevi build`, `brevi check` and `brevi run --native` never fall back, and `--strict` (from the next release) refuses the dynamic fallback too, for CI and production jobs that require the typed fast path. Files can import their sibling modules and the standard library, which Brevi builds natively itself ([what runs native](https://brevi-lang.org/docs/compatibility#what-runs-native)), and installed packages: numpy, requests, httpx, pydantic, the `anthropic` and `openai` SDKs, `mcp`, jinja2, polars, sqlite3, duckdb, tiktoken and others build with Brevi's own native copies (from v0.26.0), and the rest (pandas, yaml, ...) run in CPython inside the native program; `brevi build x.py --standalone` packs CPython and those packages into the binary (Linux). More in [Using Brevi](https://brevi-lang.org/docs/usage).

## For agents

- **MCP server:** `brevi mcp` serves `check`, `run`, `test`, `verify`, `patch`, `edit`, `show` and `guide` over stdio, or over Streamable HTTP with `brevi mcp --http [HOST:]PORT` (a bearer token is required off 127.0.0.1). Add it as the command `brevi` with the argument `mcp` (Claude Code: `claude mcp add brevi -- brevi mcp`). The agent sends plain Python as `code` once; the server keeps it, and `patch` (whole `def`s and classes by name) or `edit` (a range of lines) change only what they are sent and check again, so a retry costs only the changed lines. `run` with `cases` builds once and prints `ok` or the first differing line per case; `verify` runs the native build and CPython on the same inputs and returns a certificate when they agree. The tools sandbox by default, and from v0.27.0 a plain `brevi mcp` refuses a call that turns the sandbox off or grants anything (`--allow`, `--allow-net` and the other grant flags set what calls may ask for, `--trust-calls` restores the old behaviour); `verify`, which runs CPython, needs `sandbox: false` and so `--trust-calls`. Claude Desktop setup (it needs the full path to `brevi`), time and memory limits and troubleshooting: [For AI agents](https://brevi-lang.org/docs/agents#mcp-server).
- **Python SDK** (from v0.27.0): `pip install brevi` also gives `import brevi`. `brevi.run(code, stdin=..., limits=..., allow_read=[...])` builds and runs the program sandboxed with only what your code grants and returns its `stdout`, `stderr`, `exit_code` and `path`; `brevi.verify(code, inputs=[...])` returns the certificate; `brevi.TOOL` and `brevi.handle(tool_input)` are the `run_python` tool and its reply for a model. It talks to one `brevi mcp` process, so the build cache stays warm. The npm package has the same calls for Node (`import { run } from "brevi"`). `brevi-python` is a drop-in for `python3`: native when Brevi builds the program, CPython otherwise, with CPython's output. [packaging/pypi/README.md](packaging/pypi/README.md#python-sdk)
- **Sandbox image** (from v0.27.0): `ghcr.io/ets-public/brevi-sandbox:latest` has Brevi, the Rust toolchain, python3, the SDK and a warm build cache; `:python3` has `brevi-python` as `python3`. [templates/README.md](templates/README.md#published-image)
- **GitHub Action** (from v0.27.0): `uses: ets-public/brevi/setup@v0.28.3` puts Brevi and its toolchain on a runner, with the build cache kept between runs. [setup/README.md](setup/README.md)
- **Dev container feature** (from v0.27.0): `"features": {"ghcr.io/ets-public/features/brevi:1": {}}` in `devcontainer.json`. [devcontainer/README.md](devcontainer/README.md)
- **Skill and plugin:** [skills/brevi/SKILL.md](skills/brevi/SKILL.md) is an Agent Skills package (setup, then write plain Python and fix only the lines an error names). In Claude Code, `/plugin marketplace add ets-public/brevi` and `/plugin install brevi@brevi` add the skill and the MCP server together.
- **llms.txt:** [brevi-lang.org/llms.txt](https://brevi-lang.org/llms.txt), [llms-full.txt](https://brevi-lang.org/llms-full.txt), and every docs page as Markdown (add `.md` to its address).
- **Editor rules** for Cursor, Copilot, Windsurf and Cline in [skills/rules/](skills/rules/), and tested examples for LangChain, the OpenAI Agents SDK, CrewAI and the Vercel AI SDK in [integrations/](integrations/README.md).
- **Agent platform templates** in [templates/](templates/README.md): run the Python an agent writes on E2B, Daytona, Modal or Cloudflare Sandbox (or Docker), with only the capabilities and limits your code grants, from one image and one `run_python` tool.
- **Prompting:** ask for the plain, direct solution and say it will be compiled. Models that expect CPython hand-optimise, which costs tokens and is often slower under Brevi. Details in [For AI agents](https://brevi-lang.org/docs/agents).

## How it works

Brevi tries the fastest path first. The **typed build** works out a fixed type for every value and writes plain Rust; most saved agent programs build this way (567 of 813), and it is where the speed figures come from. A program whose values change type, or that the typed build cannot match exactly, gets the **dynamic build**: still native Rust, over values that behave like CPython's objects. Most numpy code builds with Brevi's own numpy, matching numpy's output byte for byte on AVX2 and AVX-512 CPUs with numpy on one thread (a few functions can differ in the last digit elsewhere), and so do the libraries agents reach for most (requests, httpx, pydantic, the LLM SDKs, `mcp`, jinja2, polars, sqlite3, duckdb, tokenizers, safetensors, onnxruntime; from v0.26.0). Other **library calls** (pandas, yaml, ...) run in CPython loaded into the native program. Only when none of these can build a file does `brevi run` fall back to `python3`. Every build off the typed path says which path ran and why in one JSON line (`{"path":"dynamic","line":3,"fn":"show","why":...}`), so an agent knows which function to rewrite. Integers grow past 64 bits as in Python and never silently wrap. Under `--sandbox`, and in the MCP tools, which sandbox by default, a program gets no files, network, native code or libraries unless the caller grants them (`--allow read,write`, `--allow-read data`, `--allow-write out`, `--allow-net api.example.com`, `--allow-py yaml`); the code cannot grant itself anything, a refusal is final, and a sandboxed program never falls back to `python3`. On Linux, CPython loaded under the sandbox is held to the same file grants by Landlock. The sandbox is enforced by Brevi's own generated code: there is no seccomp filter, namespace or VM, `--allow-py` and `--allow native` are full trust, Landlock is Linux-only, and no outside security review has been done yet. Treat it as a cheap limit layer for agent code you mostly trust, and run untrusted code inside a container or VM as well. `--limit cpu=2,mem=512,time=10,output=64` (MCP `limits`) stops a program at the first CPU, memory, wall-clock, output or file-size limit it reaches, and `written`, `net`, `threads`, `fds` and `files` limits are new in v0.26.0 ([threat model](docs/THREAT_MODEL.md)). Diagrams and details: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

Brevi also has its own shorter Python-like language for `.bv` files, checked fully before it runs: [BREVI.md](BREVI.md) (3,589 tokens, served by `brevi guide`), [REFERENCE.md](REFERENCE.md) and [LIBRARIES.md](LIBRARIES.md) (Rust crates and Python from Brevi). Agents finish tasks more cheaply in plain Python, so the agent tools ask for Python.

## Status: v0.28.3

v0.28.3 (18 November 2026) is a security point release on v0.28.2. Every command now refuses an option it does not have, with exit code 2 and the nearest real option named; before, `brevi build fs.py --sandbx` dropped the misspelt flag and built an unsandboxed binary. Words after `--` are the program's own and are no longer taken as Brevi options. Nothing else changed from v0.28.2.

v0.28.2 (16 November 2026) is a security and Windows point release on v0.28.1. On Linux and other unix, Brevi's temporary folders are private to the user (`$TMPDIR/brevi-UID`, owner and mode checked); before, another local user could create them first. `brevi mcp --http` bounds how much a client can make it read and hold before the token is checked. On Windows, CRLF source files, `\\?\` source paths and the cp1252 codec now give CPython's answers, which fixes the 23 Windows-only stdlib wrong answers, and a release is gated on Windows stdlib conformance as well as core. With no `rustc` on the PATH, brevi now fetches the pinned Rust it is tested with (and zig to link, if needed) on Linux x86-64 and macOS, each checked by SHA-256. The security posture is stated plainly: no external audit yet, and the sandbox is a policy layer, not an isolation boundary. Conformance numbers below are at v0.28.1 (v0.28.2's are in its release notes); speed and release-test numbers were measured at v0.28.0.

v0.28.0 (15 November 2026) brings `match` statements, metaclasses, PEP 695 generics, `bytearray` and `memoryview`, CPython 3.13's own str hashing and set order, and many more of CPython's own tests to the dynamic build, with no known wrong answer on either conformance slice. Brevi now uses only CPython 3.13 wherever it runs or asks CPython. Full list: [CHANGELOG.md](CHANGELOG.md).

- **Measured against CPython's own tests.** `tools/conformance.py` runs CPython 3.13's own tests through Brevi in two slices, at `PYTHONHASHSEED=0` so set and dict order is fixed. The core slice (`test_grammar`, `test_scope`, `test_int`, `test_list`, `test_dict`, `test_contains`, `test_binop`, `test_richcmp`, `test_set`, `test_patma`): at v0.28.1, **97.3%** of the scored suite, with 83 string-as-code tests excluded (1043 of 1072) print exactly what CPython prints, and **none** give a different answer or crash ([report](bench/conformance/report.md)); at v0.27.0 it was 250 of 311 (80.4%) with 8 different. The standard-library slice (`itertools`, `collections`, `re`, `textwrap`, `functools`, `string`, `graphlib`, `colorsys`, `deque`, `dataclasses`, `enum`): **96.9%** (1989 of 2053), **none** different ([report](bench/conformance/stdlib/report.md)); at v0.27.0, 250 of 554 (45.1%) with 22 different. Both left out the tests that run a string as code (83 core, 6 stdlib), which a sandboxed native binary refuses by design. At v0.28.1, counting only native matches whose program printed something to compare (assertion-bearing), core is 88.2% (945 of 1072) and stdlib 94.3% (1937 of 2053); 2.6% (27) and 1.2% (23) of the native matches took the typed fast path, the rest the dynamic build. 2 stdlib matches ran through the CPython bridge and are not counted. The rest are refused at build time with a reason, or cannot be cut into a standalone script yet. That covers the files scored, not all of CPython: more test files are measured for planning (`tools/conformance.py --candidates`) but not scored yet. Brevi's aim is that a build either matches CPython or is refused; every different answer is a bug against that aim, and those are what the next releases fix first. For your own program, `brevi verify x.py --input tests/` runs the native build and CPython on the same inputs and prints a certificate (file and input hashes, path, speedup) when stdout, exit code and stderr agree, or the first line that differs (from v0.26.0). From v0.28.0, tests that run a string as code (`compile`, `exec`, `eval`) are counted as **refused by design**: a sandboxed native binary refuses to run new code from a string on purpose. They are named in [compat/conformance_by_design.txt](compat/conformance_by_design.txt) (83 core, 6 stdlib), left out of the denominator, and their count is printed beside every score. Tests that need a str holding a lone surrogate ([#450](https://github.com/ets-public/brevi/issues/450)) or call CPython-private helpers ([#481](https://github.com/ets-public/brevi/issues/481)) would be named in [compat/conformance_needs_string_model.txt](compat/conformance_needs_string_model.txt) and [compat/conformance_cpython_private.txt](compat/conformance_cpython_private.txt) and stay in the denominator as not matched; both lists are empty at v0.28.0, because those tests match. Tests of CPython's own machinery (crash checks in a subprocess, `multiprocessing`, `_testinternalcapi`) and tests CPython itself skips on the reference build are left out with their reason, the same as `cpython_only` tests. Outside the aim altogether: text on stderr, and output CPython itself changes between runs (the order of a set of strings, `__del__` timing).
- **Regression set: saved agent programs.** On unchanged Python files, Brevi builds all 591 saved agent programs and all 18 large programs (chess, a text adventure, an interpreter), and every one matches CPython's output and exit status. These are solutions to our own benchmark tasks, kept once they passed, so this is a check that nothing regresses, not a measure of how much Python works. Codon builds 206 of the 591 and gets 36 of those wrong (Codon changes some Python behaviour on purpose, such as 64-bit integers); Nuitka matches all of them ([bench/compare](bench/compare/README.md)).
- **Faster, on the typed path.** On 12 compute-heavy programs (n-body, spectral norm, fannkuch, binary trees, ...) the default build runs 1.6x to 37.9x faster than CPython, median 9.3x; `--release` is 2.1x to 38.8x, median 9.8x (measured 12 Nov 2026 on a Ryzen 9 5950X under WSL2; on 3 Nov, on a 4-core container, `--release` was roughly level with Codon where Codon builds). Those numbers come from the typed build. A program whose values change type gets the dynamic build, which runs at about CPython's speed or a little slower: 0.5x to 1.4x CPython on the same 12 programs, median 0.85x; 567 of 813 saved agent programs take the typed build, but only 25 of the 230 native builds of CPython's tests do. `brevi check` answers in well under a second (median 144 ms for a new file), and an unchanged program is re-checked from cache in about 10 ms.
- **About the same cost for agents.** In the release test, Claude Sonnet and Haiku solve 24 tasks in plain Python, once checked by CPython and once by Brevi, and the whole API bill is compared. The bar: every task passes, Brevi's cost is within 10% of CPython's, and at least 90% of final programs build natively. v0.28.0 met it over two runs: 1.08x for Sonnet (95% range 1.02x to 1.14x over tasks) and 1.00x for Haiku (0.95x to 1.06x), every task passed on both sides, and 95 of 96 final programs built natively; earlier releases ranged from 1.03x to 1.09x for Sonnet. So Brevi does not save model cost on ordinary tasks; it costs about the same, or a few percent more. In these runs, when Brevi cannot build a file it runs with CPython and the model does not see Brevi's build error, as with `brevi run`. On four heavy tasks the Brevi side alone was told its code is compiled and should be written plainly, and cost 0.58x (Sonnet) and 0.53x (Haiku); CPython has not yet been run with the same line, so that figure does not yet show what Brevi itself saves.
- **Known gaps:** `eval` of text known only at run time is not built natively yet (`eval` of literal text is, from v0.27.0; `match` statements are, from v0.28.0) (`async` / `await` is, from v0.26.0), classes built on a library's classes run in CPython as a whole, `--standalone` is Linux and Windows only (not macOS), and on Windows `--limit cpu`, `file` and `fds` are refused (`mem` is held by a Job Object) (library calls, generators, `async` and Brevi's own sqlite3, requests/httpx, duckdb, onnxruntime and MCP work there from v0.27.0). See [What works, and limits](https://brevi-lang.org/docs/compatibility) and the [roadmap](https://brevi-lang.org/docs/roadmap).

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
| `tools/conformance.py` | CPython's own test suite, cut into standalone scripts and built with Brevi; [latest result](bench/conformance/report.md); the released numbers and how to quote them: [docs/NUMBERS.md](docs/NUMBERS.md) |
| `tools/compat.py`, `compat/baseline.txt` | Every bench solution that ever passed; must keep passing on new toolchains |
| `integrations/` | Examples for LangChain, OpenAI Agents SDK, CrewAI and Vercel AI SDK over `brevi mcp` |
| `templates/` | Agent sandboxes on E2B, Daytona, Modal, Cloudflare and Docker, running code under Brevi's sandbox and limits |
| `grammar/`, `tools/grammar_check.py`, `tools/lark_to_gbnf.py` | Grammar for constrained decoding, and its checks |
| `tools/upstream_watch.py` | Opens review issues for new Rust, Python and crate releases |
| `.github/` | Daily toolchain matrix, upstream watch, Dependabot (see "Staying current") |

## Staying current

Brevi must never fall behind Rust or Python, and code that once passed must never silently break.

- **Toolchains** (`.github/workflows/toolchains.yml`): builds Brevi and reruns `tests/check.sh` plus `tools/compat.py check`
  (all 1,900+ saved Brevi, Python and Rust solutions that once passed). Daily on Rust stable with the newest and the next
  Python pre-release; Mondays also on Rust beta and nightly, where nightly reports deprecation and future-incompatibility
  warnings in generated Rust and the newest edition, about 6 to 12 weeks before they reach stable. PRs test only the Rust
  pinned in `.github/rust-version` (as do releases and the other workflows; the daily run tests the latest stable), skip drafts and docs-only changes, and rerun the old solutions only when the compiler or baseline changed.
  A toolchain already tested at the same commit is skipped. Failures open a `toolchain` issue. To save Actions minutes,
  run `sh tools/ci_local.sh` (the same checks) before merging.
- **Upstream releases** (`upstream.yml`, twice a week): one `upstream` issue per new Rust stable, Python release or
  newer crate pinned in tests/examples, listing the new language features and stabilized APIs with a review checklist.
- **Dependabot**: Cargo dependencies and GitHub Actions versions, weekly.

After a change that makes more solutions pass, run `python3 tools/compat.py baseline` to lock them in.
Never remove a line from `compat/baseline.txt` to make a check pass.

## Contributing and security

See [CONTRIBUTING.md](CONTRIBUTING.md) for bug reports and pull requests, and [SECURITY.md](SECURITY.md) to report a vulnerability privately and for what the sandbox does and does not cover. For anything else, email [support@brevi-lang.org](mailto:support@brevi-lang.org).

## License

Apache License 2.0. See [LICENSE](LICENSE).

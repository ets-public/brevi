# FAQ
<!-- description: Short answers for people shipping a Python MCP server or CLI: what Brevi builds, whether the run machine needs Python, binary size, start time, which servers build, and how it differs from PyInstaller, Nuitka and zipapp. -->

## What is Brevi?

A compiler, written in Rust, that turns a small Python MCP server or CLI script into one native binary. The machine that runs the binary needs no Python. It also starts fast: the first tool call of the official `mcp-server-time` 2026.11.4 is answered in 35 ms, against 730 ms on CPython 3.13 (Brevi 0.26.0, Linux x86-64, Intel Xeon 2.10 GHz, 4 cores, median of 10 runs, same output byte for byte). (Its current release, 2026.11.18, no longer builds natively: it takes the CPython bridge.) Most real MCP servers need the bridge today: of the ten most-downloaded Python MCP servers on PyPI, built unchanged on 19 November with Brevi 0.28.3, none built natively, and all ten took 1.00x to 1.16x CPython's time to the first tool call ([report](repo:bench/showcase/REAL_SERVERS.md)).

```
brevi build server.py --release -o my-server
claude mcp add my-server -- ./my-server
```

Brevi is a preview, and not every server builds. [Ship an MCP server or CLI as one binary](/docs/bundle-mcp) is the full guide.

## Does the machine that runs it need Python?

Not for programs Brevi builds itself, which includes the official `mcp` SDK, sqlite3, tiktoken and the rest of what [What works](/docs/compatibility) lists. The binary links no libpython.

An import Brevi does not build itself (pandas, yaml, ...) runs in CPython through a bridge. The build says so with a `{"path":"pybridge",...}` line. That binary needs Python 3.13 and the packages it was built against (the build venv's site-packages) on the run machine, or `--standalone`, which packs CPython and the packages into the binary (Linux and Windows only; about 13 MB for the stdlib alone, 30 to 45 MB with numpy) and unpacks them to `~/.cache` on first run, which takes 1 to 3 s. `--standalone` misses packages imported lazily, and broke 4 of the 10 popular servers we tested ([report](repo:bench/showcase/REAL_SERVERS.md)). The fast start is lost on the bridge.

## How big is the binary?

On the same machine and version: 19.2 MB for the official `mcp-server-time`, 18.0 MB for an SQLite MCP server on the official `mcp` SDK, and 11.5 MB for a one-file token counter that uses tiktoken. Sizes are not tuned yet (no LTO or symbol stripping), and work to shrink them is planned.

## How fast does it start?

Measured on Brevi 0.26.0 (commit 69caf93, 10 Nov 2026), Linux x86-64, Intel Xeon 2.10 GHz, 4 cores, median of 10 runs, against CPython 3.13, same output byte for byte:

| Program | Brevi | CPython | Binary |
|---|---|---|---|
| `mcp-server-time` 2026.11.4, unchanged, first tool call | 35 ms | 730 ms | 19.2 MB |
| SQLite MCP server, official `mcp` SDK | 27 ms | 718 ms | 18.0 MB |
| Token counter (tiktoken), one file | 33 ms | 375 ms | 11.5 MB |

The win is start-up and import time, not throughput. The same token counter over a whole source tree is only 2.2x faster (395 ms against 872 ms). The dynamic build most programs take runs at about CPython's speed (0.5x to 1.4x, median 0.85x). [Benchmarks](/docs/benchmarks) has the method.

## Which servers build?

Servers and scripts that use the standard library, the official `mcp` SDK and the libraries [What works](/docs/compatibility) lists. When one does not build, `brevi build` says why in one JSON line, naming the line and function. Run `brevi check server.py` first to see this without building. Servers that use sampling, elicitation, auth, or the SSE and streamable HTTP apps run on CPython's `mcp` through the bridge, and servers written for the older `mcp.server.fastmcp.FastMCP` import do not build natively yet.

## What if my server imports something Brevi lacks?

It still builds when the import can go through the CPython bridge (see the second question), but the binary then needs Python 3.13 and the packages it was built against (the build venv's site-packages) on the run machine, or `--standalone` (which can miss lazily imported packages), and it no longer starts fast. That is the case for most real servers today ([report](repo:bench/showcase/REAL_SERVERS.md)). If you need a binary with no Python at all, check the build output for the `pybridge` line and replace that import.

## Which platforms?

- **Linux:** x86-64 and ARM64. Binaries link the system glibc dynamically (no static or musl build yet), so build on the oldest Linux you target.
- **macOS:** Brevi is released for macOS, but CI does not test the programs it builds there, only the install. `--standalone` and `--limit mem` are refused on macOS.
- **Windows:** beta. Limits are in [Known issues](repo:docs/KNOWN_ISSUES.md#windows-beta).

## Is it safe for a long-running server?

Be careful. A built program does not free some memory yet (reference cycles, some typed objects). That is fine for short sessions, such as an MCP server a coding agent starts and stops. Watch the resident memory of a server that runs for days. A fix is in progress.

## How is this different from PyInstaller, Nuitka and shiv or zipapp?

- **PyInstaller** packs your script, its imports and a CPython interpreter into one file or folder. The program still runs on CPython, so start time is CPython's.
- **shiv and zipapp** bundle your code and dependencies into a `.pyz` archive. It needs a Python on the run machine, and it starts as CPython does.
- **Nuitka** compiles your Python to C and then to a native binary. It also links libpython, and it supports far more of Python than Brevi does today.
- **Brevi** compiles through Rust and does not link libpython for programs it builds itself. That is what removes the Python install and the interpreter start-up.

We have not benchmarked these tools, so this page gives no numbers for them. Brevi supports less of Python than they do: it is a preview.

## Do I have to learn a new language?

No. Write Python. Brevi also has its own language for `.bv` files, but you do not need it to build Python.

## Why does it need Rust on the build machine?

Brevi writes Rust and calls `rustc`. With no `rustc` on PATH, the first build fetches Brevi's pinned Rust (about 300 MB, once). Only the build machine needs it. `uvx brevi build server.py --release -o my-server` installs and builds in one command; other ways are on [Install](/docs/install).

## Will my program give the same answer?

That is the aim, and usually yes, but not always yet. Brevi refuses at build time what it cannot match. For a program that matters, run `brevi verify x.py --input tests/`: it runs the native build and CPython on the same inputs and certifies them only if they agree. [What works, and limits](/docs/compatibility#in-numbers) has the conformance numbers. If you find a difference, it is a bug, so please report it.

## Can coding agents use it too?

Yes. `brevi run --verify main.py` runs a program and checks it against CPython, `brevi check` gives one-line JSON errors, and `brevi mcp` serves those tools to an agent. See [For AI agents](/docs/agents).

## Is it safe to run code an agent wrote?

Code you trust, yes; code that may be hostile, not on Brevi's sandbox alone. **No external security audit yet**: we have not had an external audit; we are seeking funding for one. Until then, do not run hostile code under the sandbox.

**The sandbox is a policy layer, not an isolation boundary.** Under `--sandbox` (the default in the MCP tools and the SDKs, except `verify`, which runs CPython) a program may touch only what the caller grants (`--allow`, `--allow-net`, `--allow-read`, `--allow-write`, `--allow-py`). Brevi's own checks and runtime enforce it, with no container or VM around the program, so **run untrusted code in a container or VM** and use the sandbox as a second layer inside it. **`--allow-py` and `--allow native` are full trust**: grant them only to code you would run without a sandbox. See [Sandbox](/docs/usage#sandbox).

## Is it free?

The compiler is open source under Apache-2.0 and will stay that way.

## Can I install it with pip, npm or Homebrew?

pip and npm, yes: `pip install brevi` or `npm install -g brevi`, or `uvx brevi ...` without installing. The crate is on crates.io. Homebrew not yet. See [Install](/docs/install).

## Who makes Brevi?

Brevi is developed largely by AI coding agents (Claude) working under the direction of its maintainer, with every change tested against the benchmarks and regression suites before it is merged.

## How do I get help or contact you?

Email [support@brevi-lang.org](mailto:support@brevi-lang.org) for questions, licensing or anything you would rather not post in public. Bugs and feature requests are best as a GitHub issue, and security reports go through [SECURITY.md](repo:SECURITY.md).

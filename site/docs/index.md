# Brevi documentation
<!-- description: Brevi documentation (experimental, archived): what Brevi builds, how, and why real MCP servers did not build natively. -->

> **Brevi is experimental and archived.** It works on small programs, but of the ten most-downloaded Python MCP servers on PyPI, built unchanged on 19 November 2026 with Brevi 0.28.3, none built natively: all ten needed the CPython bridge, and so Python on the run machine, and started at 1.00x to 1.16x CPython's time to the first tool call. The [real-server report](repo:bench/showcase/REAL_SERVERS.md) has the data. We are not developing Brevi further and do not recommend it for new projects. The pages below describe what it does on the programs it does build.

Brevi compiles Python through Rust into one native file. For a program it builds natively, the machine that runs it needs no Python, no virtualenv and no packages.

```
$ brevi build server.py --release -o time-mcp     # build once
$ claude mcp add time -- /full/path/to/time-mcp   # register it
```

The official `mcp-server-time` 2026.11.4, built unchanged this way, answers its first tool call in 35 ms where CPython 3.13 takes 730 ms, from one 19.2 MB file that needs no Python (Brevi 0.26.0, Linux x86-64, Intel Xeon at 2.10 GHz with 4 cores, median of 10 runs; [Showcases](/docs/showcases)). The win is start-up and import time: the dynamic build most servers take runs at about CPython's speed once started (median 0.85x on our compute benchmarks). (Its current release, 2026.11.18, no longer builds natively: it takes the CPython bridge.)

Brevi is written in Rust and released under the Apache-2.0 license.

## Who it is for

- **People who maintain a small Python MCP server or CLI** and want users to run it without installing Python, and to start it fast. Start with [Ship an MCP server or CLI](/docs/bundle-mcp).
- **Coding agents** that write such a server: the same compiler gives one-line JSON errors and `brevi run --verify` checks a run against CPython 3.13. Start with [For coding agents](/docs/agents).
- **Anyone curious how a Python-to-Rust compiler can stay exact**: [Architecture](/docs/architecture) and [What builds, and limits](/docs/compatibility).

## Before you ship

- **Imports Brevi does not build itself** (pandas, yaml, ...) run in CPython inside the binary. That build needs Python 3.13 and the packages it was built against (the build venv's site-packages) on the run machine, or `--standalone` (Linux and Windows) to carry them, which misses lazily imported packages. Either way it loses the fast start. The build says so in one line. Most real MCP servers need the bridge today: of the ten most-downloaded Python MCP servers on PyPI, built unchanged on 19 November with Brevi 0.28.3, none built natively, and all ten took 1.00x to 1.16x CPython's time to the first tool call ([report](repo:bench/showcase/REAL_SERVERS.md)).
- **Linux binaries link glibc**: build on the oldest Linux you target. No static build yet.
- **macOS**: released, but programs built there are not yet tested in CI. **Windows** is a beta.
- **Long-running servers**: a built program does not yet free some memory, such as reference cycles; watch the memory of a server that runs for days.
- **Security**: a built binary has the same access as any program. Brevi's `--sandbox`, for running code an agent wrote, is a policy layer, not an isolation boundary: run untrusted code in a container or VM, and know that `--allow-py` and `--allow native` are full trust. No external security audit yet ([Security](https://github.com/ets-public/brevi/blob/main/SECURITY.md#where-brevi-stands)).

## Where to go next

<div class="cards">
<a class="card" href="/docs/bundle-mcp"><h3>Ship an MCP server or CLI</h3><p>Build, check against CPython, register and ship, step by step.</p></a>
<a class="card" href="/docs/install"><h3>Install</h3><p>Brevi for the machine that builds: uv, the install script, pip, npm, Docker, or from source.</p></a>
<a class="card" href="/docs/showcases"><h3>Showcases</h3><p>Real servers and CLIs built unchanged: start time, size, and whether they need Python.</p></a>
<a class="card" href="/docs/compatibility"><h3>What builds, and limits</h3><p>What runs native, what needs CPython, what is refused, and the conformance numbers.</p></a>
<a class="card" href="/docs/usage"><h3>Using Brevi</h3><p>Commands, flags, which path a file took, errors and standalone builds.</p></a>
<a class="card" href="/docs/agents"><h3>For coding agents</h3><p>Brevi's own MCP server, Agent Skill, Claude Code plugin and llms.txt.</p></a>
</div>

## Status

Brevi is a preview, at version __VERSION__. Not every server builds natively yet; when one does not, the build names the line that stopped it, and the [roadmap](/docs/roadmap) says what is being fixed next. Run it with `uvx brevi build server.py --release -o my-server` (no Rust needed: the first build fetches the pinned Rust once), or install it with the script, `pip install brevi`, `npm install -g brevi`, `cargo install brevi` or from the GitHub release ([Install](/docs/install)).

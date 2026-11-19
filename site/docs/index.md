# Brevi documentation
<!-- description: Brevi documentation: turn a Python MCP server or CLI into one native binary, install, build, ship, and what builds today. -->

**Turn your Python MCP server or CLI into one fast binary.** Brevi compiles the Python you already have, imports included, through Rust into one native file. The machine that runs it needs no Python, no virtualenv and no packages. New here? [Ship an MCP server or CLI](/docs/bundle-mcp) goes from `server.py` to a registered binary in a few commands.

```
$ brevi build server.py --release -o time-mcp     # build once
$ claude mcp add time -- /full/path/to/time-mcp   # register it
```

The official `mcp-server-time`, built unchanged this way, answers its first tool call in 35 ms where CPython 3.13 takes 730 ms, from one 19.2 MB file that links no libpython (Brevi 0.26.0, Linux x86-64, Intel Xeon at 2.10 GHz with 4 cores, median of 10 runs; [Showcases](/docs/showcases)). The win is start-up and import time: the dynamic build most servers take runs at about CPython's speed once started (median 0.85x on our compute benchmarks).

Brevi is written in Rust and released under the Apache-2.0 license.

## Who it is for

- **People who maintain a small Python MCP server or CLI** and want users to run it without installing Python, and to start it fast. Start with [Ship an MCP server or CLI](/docs/bundle-mcp).
- **Coding agents** that write such a server: the same compiler gives one-line JSON errors and `brevi run --verify` checks a run against CPython 3.13. Start with [For coding agents](/docs/agents).
- **Anyone curious how a Python-to-Rust compiler can stay exact**: [Architecture](/docs/architecture) and [What builds, and limits](/docs/compatibility).

## Before you ship

- **Imports Brevi does not build itself** (pandas, yaml, ...) run in CPython inside the binary. That build needs Python's shared library on the run machine, or `--standalone` (Linux and Windows) to carry it, and loses the fast start. The build says so in one line.
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

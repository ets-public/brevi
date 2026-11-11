# Brevi documentation
<!-- description: Brevi documentation: install, use, connect agents, and how the compiler works. -->

Brevi is a compiler built for AI coding agents. The agent writes ordinary Python, and Brevi builds it through Rust into a native program. On our 12 compute benchmarks the default build runs 1.7x to 39.5x faster than CPython (median 8.5x). Brevi aims to print exactly what CPython prints and to refuse what it cannot match; on CPython 3.13's own tests it matches 80% today and still gets 3% wrong, so it is a preview, not a drop-in `python3` ([the numbers](/docs/compatibility#in-numbers)). When Brevi cannot build something, it says so in one short JSON line the agent can act on.

Brevi is written in Rust and released under the Apache-2.0 license.

```
$ brevi run top.py < text.txt        # build natively, then run
the: 3
and: 2
cat: 1
```

## Who it is for

- **Coding agents** that write Python to solve a task: Brevi takes the place of `python3` in the loop. The agent needs no guide, and gets faster programs and shorter errors. Start with [For AI agents](/docs/agents).
- **People who run agent-written code**: one native binary to ship, the same output as CPython, and a sandbox the program declares. Start with [Install](/docs/install) and [Using Brevi](/docs/usage).
- **Anyone curious how a Python-to-Rust compiler can stay exact**: [Architecture](/docs/architecture) and [What works, and limits](/docs/compatibility).

## What makes it different

- **AI-first.** Every design choice is measured by what it costs a model to get a working program: tokens written, errors read, retries. The [release test](/docs/benchmarks#the-release-test) has Claude models solve 24 tasks in Python, run once by CPython and once by Brevi, and Brevi must cost no more.
- **Fast.** Most Python builds into plain Rust with fixed types. Programs whose types change at run time build over dynamic values, still native.
- **Safe.** Rust's memory safety underneath, integers that never silently overflow, and a declared sandbox for file access and native code.
- **Exact.** A program Brevi builds gives CPython's output, or Brevi refuses at build time. It does not guess. On 591 saved agent programs, every build matched CPython byte for byte.

## Where to go next

<div class="cards">
<a class="card" href="/docs/install"><h3>Install</h3><p>Release binaries, the install script, Docker, or from source.</p></a>
<a class="card" href="/docs/usage"><h3>Using Brevi</h3><p>Commands, flags, errors, the CPython fallback, sandbox and GUI tests.</p></a>
<a class="card" href="/docs/agents"><h3>For AI agents</h3><p>MCP server, Agent Skill, Claude Code plugin, llms.txt and prompting advice.</p></a>
<a class="card" href="/docs/architecture"><h3>Architecture</h3><p>The pipeline from source to native binary, with diagrams.</p></a>
<a class="card" href="/docs/compatibility"><h3>What works, and limits</h3><p>Python coverage, libraries, big integers and known gaps.</p></a>
<a class="card" href="/docs/benchmarks"><h3>Benchmarks</h3><p>Release test, model cost, and Brevi against Codon and Nuitka.</p></a>
</div>

## Status

Brevi is a preview, at version __VERSION__. It builds most real-world agent-written Python natively today, and the [roadmap](/docs/roadmap) lists what is still missing. Package-manager installs (pip, npm, Homebrew) and the hosted service at [brevi.run](https://brevi.run) are coming; for now, install from the GitHub release.

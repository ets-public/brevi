# Brevi: fast, safe native builds of AI-written Python

Brevi is a compiler built for AI coding agents. The agent writes the plain Python it already writes best; Brevi builds it through Rust into a native binary that prints exactly what CPython prints and runs 1.7x to 40x faster with the default build. Anything it cannot build comes back as one short JSON line the agent can act on.

```
pip install brevi        # or run it without installing: uvx brevi mcp
```

```
$ printf 'print(sum(i * i for i in range(10)))\n' > main.py
$ brevi run main.py                     # build natively (cached), then run
285
$ brevi check main.py                   # build without running: errors only
$ brevi build main.py -o app --release  # an optimised binary to keep or ship
$ brevi mcp                             # MCP server for agents: check, run, test, verify, patch, edit, show, guide
```

A sure mistake is reported before anything runs, as one line with a fix:

```
{"line":8,"msg":"name 'totl' is not defined","fix":"did you mean total?"}
```

Brevi compiles through `rustc`, so a Rust toolchain must be on the PATH ([rustup.rs](https://rustup.rs)).

- **For agents:** add the MCP server as the command `brevi` with the argument `mcp` (Claude Code: `claude mcp add brevi -- brevi mcp`). Its tools sandbox by default. [For AI agents](https://brevi-lang.org/docs/agents)
- **Same answers as CPython:** every saved agent program and large test program matches CPython's output and exit status. Imports of installed packages (numpy, pandas, requests, ...) work; most numpy code builds natively.
- **Public preview:** Brevi works today and is tested against CPython on every change, but expect the CLI to change between releases.

[Docs](https://brevi-lang.org/docs/) · [Source](https://github.com/ets-public/brevi) · [Changelog](https://github.com/ets-public/brevi/blob/main/CHANGELOG.md) · [Issues](https://github.com/ets-public/brevi/issues) · Apache-2.0

<!-- Lets the MCP registry confirm this package belongs to the server io.github.ets-public/brevi. -->
mcp-name: io.github.ets-public/brevi

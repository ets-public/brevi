# Quickstart
<!-- description: From install to a shipped MCP server binary and a CLI binary in a few commands: build one file with no Python needed on the run machine, register it with Claude Code, then the short loop for coding agents. -->

This page takes a small Python MCP server and a CLI script to two native binaries. The machine that runs them needs no
Python, as long as the program uses only what Brevi builds itself. Brevi is a preview, so not every server builds; when one
does not, the build says why in one JSON line. The full guide is
[Ship an MCP server or CLI as one binary](/docs/bundle-mcp).

## 1. Install

<!-- install-line: the one place this page names the install command; swap it when the install changes -->
With [uv](https://docs.astral.sh/uv/) installed, nothing else is needed on the build machine:

```
uvx brevi build server.py --release -o my-server
```

That is the build command of step 3. With no `rustc` on PATH, the first build fetches Brevi's pinned Rust (about 300 MB,
once). To keep `brevi` on your PATH, run `uv tool install brevi`. The install script, pip, npm, Docker and the release
archives: [Install](/docs/install). The commands below say `brevi`; write `uvx brevi` instead if you did not install it.
<!-- /install-line -->

```
$ brevi --version
brevi __FULLVERSION__
```

## 2. Write a server

Plain Python on the official `mcp` SDK, in the style of Brevi's own showcases. Save it as `server.py`:

```python
from mcp.server.mcpserver import MCPServer

mcp = MCPServer("greeter")


@mcp.tool()
def greet(name: str) -> str:
    """Say hello to someone."""
    return f"Hello, {name}!"


if __name__ == "__main__":
    mcp.run()
```

## 3. Build it and register it

```
brevi build server.py --release -o my-server
claude mcp add my-server -- "$PWD/my-server"
```

If the build printed no path line with a `"py"` list (such as `{"path":"pybridge",...}`), `my-server` is one file that needs no Python. Claude Code starts it over stdio like any other MCP server. On the
official `mcp-server-time` 2026.11.4, the first tool call is answered in 35 ms against 730 ms on CPython (Brevi 0.26.0, Linux
x86-64, Intel Xeon 2.10 GHz, 4 cores, median of 10 runs, CPython 3.13); the binary is 19.2 MB, with sizes not tuned yet. (Its current release, 2026.11.18, no longer builds natively: it takes the CPython bridge.)

## 4. Build a CLI

```
brevi build tool.py --release -o tool
./tool ARGS
```

## Limits to know before you ship

- An import Brevi does not build itself (pandas, yaml, ...) runs in CPython through a bridge, shown at build as
  `{"path":"pybridge",...}`. That binary needs Python 3.13 and the packages it was built against (the build venv's site-packages) on the run machine, or `--standalone` (Linux and Windows
  only; it misses lazily imported packages), and it loses the fast start. Most real MCP servers need the bridge today: of the ten most-downloaded Python MCP servers on PyPI, built unchanged on 19 November with Brevi 0.28.3, none built natively, and all ten took 1.00x to 1.16x CPython's time to the first tool call ([report](repo:bench/showcase/REAL_SERVERS.md)).
- Linux binaries link the system glibc dynamically: build on the oldest Linux you target.
- macOS programs are not tested by CI, and Windows is beta ([Known issues](repo:docs/KNOWN_ISSUES.md#windows-beta)).
- The speed win is start-up, not throughput: most programs run at about CPython's speed (median 0.85x).
- A built program does not free some memory yet; watch a server that runs for days.

## For coding agents

Brevi also serves agents that write Python. The agent writes plain Python and runs it with `brevi run`:

```
$ brevi check main.py
{"line":16,"msg":"name 'totl' is not defined","fix":"did you mean total?"}
```

- **One-line errors.** `brevi check` checks the whole program before anything runs. A sure mistake comes back as one JSON
  line with the line number and a fix, with no traceback to read. The MCP `check` and `run` tools answer the same way.
- **Verified runs.** `brevi run --verify main.py < input.txt` compares the native run with CPython 3.13 and prints
  `{"verify":"identical",...}` on stderr when they agree. When they differ, you get CPython's answer and a
  `{"verify":"differs"}` line. `--verify` needs a CPython 3.13, found as `python3`, through `uv`, or named by `BREVI_PYTHON`.
- **When Brevi cannot build a file,** `brevi run` runs it with CPython and prints one `{"path":"cpython",...}` line naming
  the line that stopped the build.
- **MCP:** `claude mcp add brevi -- brevi mcp` serves `check`, `run`, `verify` and the edit tools. The sandbox is on by
  default there, and it is a policy layer, not an isolation boundary: run untrusted code in a container or VM
  ([threat model](__GITHUB__/blob/main/docs/THREAT_MODEL.md)). See [For AI agents](/docs/agents).
- **Python SDK:** `pip install brevi`, then `brevi.run(code, stdin=...)` and `brevi.verify(code, inputs=[...])`.

`--verify` checks your program on your inputs; it is not a claim that all of Python works
([What works, and limits](/docs/compatibility)).

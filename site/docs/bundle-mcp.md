# Ship an MCP server or CLI as one binary
<!-- description: Build a Python MCP server or command-line tool into one native binary with Brevi, check it against CPython, register it with an MCP client, and ship it to machines with no Python. -->

This guide takes a Python MCP server, or a command-line tool, and turns it into one file you can copy to another machine and run there with no Python, no virtualenv and no packages installed. The source stays as it is.

## What you need

- **On the machine that builds:** Brevi. With [uv](https://docs.astral.sh/uv/), `uvx brevi ...` runs it without installing anything else; other ways are on [Install](/docs/install). If there is no Rust on the machine, the first build fetches Brevi's pinned Rust release once (about 300 MB), checked against SHA-256 sums built into Brevi.
- **On the machine that runs it:** nothing, for a native build. On Linux the binary links the system's glibc, so build on the oldest Linux you plan to run it on (or in a container of that distribution). There is no fully static Linux build yet.

## 1. Start from your server

Any server on the official [`mcp` Python SDK](https://github.com/modelcontextprotocol/python-sdk) that uses its `MCPServer` API over stdio is a good candidate. This one is enough to follow along:

```python
# server.py
from mcp.server.mcpserver import MCPServer

server = MCPServer("words")


@server.tool()
def count_words(text: str) -> int:
    """How many words the text has."""
    return len(text.split())


if __name__ == "__main__":
    server.run()
```

Check that it runs with CPython first (`python3 server.py` with the `mcp` package installed). Brevi aims to give exactly CPython's answers, so CPython is the reference.

## 2. Build it

```bash
brevi build server.py --release -o words-mcp
```

`--release` optimises fully; use it for any binary you ship. The build compiles your server, Brevi's native copy of the `mcp` SDK and pydantic, and the standard library through Rust into `words-mcp`. A file can import its sibling modules and packages as usual; Brevi joins them into one program.

Watch stderr. When the whole server builds natively, the build prints nothing, or one `{"path":"dynamic",...}` line (still native, see [Which path ran](/docs/usage#which-path-ran)). Two other outcomes matter for shipping:

| Build prints | What it means for the run machine |
| --- | --- |
| nothing, or `{"path":"dynamic",...}` with no `"py"` list | Nothing to install. This is the fast-start build. |
| `{"path":"pybridge","py":[...]}`, or any path line with a `"py"` list | The listed imports run in CPython inside the binary. The run machine needs Python 3.13 and the packages it was built against (the build venv's site-packages), or see [step 5](#5-when-an-import-needs-cpython). |
| `{"line":N,"msg":...}` and no binary | Brevi cannot build that line yet. The message names it. |

To confirm the binary needs no Python, read that path line: a build needs no Python only when it printed nothing, or a path line with no `"py"` list. `ldd` cannot tell you: a bridged binary loads Python at run time and does not show it in `ldd`.

## 3. Check it against CPython

`brevi verify` runs the native build and CPython on the same MCP session and prints a certificate when they agree. Write the session as one JSON-RPC message per line:

```jsonl
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"check","version":"1"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/list"}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"count_words","arguments":{"text":"one two three"}}}
```

```bash
brevi verify server.py --release --input session.jsonl
```

Each request is sent once the reply before it is out, so the answer does not depend on timing. This step needs `python3` (3.13) with `mcp` installed on the build machine; the binary you ship does not. More in [Verify against CPython](/docs/usage#verify-against-cpython).

## 4. Register and ship it

With Claude Code:

```bash
claude mcp add words -- /full/path/to/words-mcp
```

Any other MCP client takes the binary as the server's command, with no arguments unless your server reads some. Claude Desktop and most clients need the full path. To ship it, copy the one file: attach it to a GitHub release, put it in a container image, or send it to the people who run it. They do not need Python, pip or uv.

## 5. When an import needs CPython

If the build printed `{"path":"pybridge",...}`, some imports run in CPython inside the binary. You have three choices:

- **Ship it with Python 3.13 and the packages it was built against (the build venv's site-packages) on the run machine.** It works, but start-up then includes loading CPython and those imports, so most of the fast start is gone.
- **Build with `--standalone`** (Linux and Windows, not macOS). It packs CPython, the standard library and the imported packages into the binary: about 13 MB more with the standard library only, 30 to 45 MB with numpy. The first run unpacks them to `~/.cache/brevi/py` (1 to 3 s); later runs start from there. Still no Python install on the run machine, but no fast start, and it misses packages a server imports lazily (inside a function): those fail at run time. It broke 4 of the [10 popular servers we tested](repo:bench/showcase/REAL_SERVERS.md), so run your server's whole session on the standalone build before you ship it.
- **Replace the import** with one Brevi builds natively, if the dependency is small. [What builds, and limits](/docs/compatibility) lists the native libraries.

Servers that use sampling, elicitation, roots, auth, or the SSE and streamable HTTP apps run on CPython's `mcp` today, and servers written for the older `mcp.server.fastmcp.FastMCP` import do not build natively yet.

## A command-line tool

The same steps work for a CLI on `argparse`:

```bash
brevi build tool.py --release -o tool
./tool --help
brevi verify tool.py --release --input cases/ -- --flag value   # optional: check it against CPython
```

A token counter on tiktoken built this way prints the count of one file in 33 ms, where CPython takes 375 ms, most of it spent starting and importing tiktoken ([Showcases](/docs/showcases#token-counting)).

## What to expect

On the official `mcp-server-time` 2026.11.4, built unchanged, the first tool call is answered in 35 ms against CPython 3.13's 730 ms, from a 19.2 MB binary that links no libpython (Brevi 0.26.0, Linux x86-64, Intel Xeon at 2.10 GHz with 4 cores, median of 10 runs). [Showcases](/docs/showcases) has the method and the other servers. (Its current release, 2026.11.18, no longer builds natively: it takes the CPython bridge.)

Most real MCP servers need the bridge today: of the ten most-downloaded Python MCP servers on PyPI, built unchanged on 19 November with Brevi 0.28.3, none built natively, and all ten took 1.00x to 1.16x CPython's time to the first tool call ([report](repo:bench/showcase/REAL_SERVERS.md)).

Limits today:

- **Start-up is the win, not throughput.** Most servers build over dynamic values, which run at about CPython's speed (median 0.85x on our compute benchmarks). A tool call that waits on the network or a model will not get faster.
- **Binary size is not tuned yet.** Expect 10 to 20 MB for an MCP server. Smaller release builds are on the [roadmap](/docs/roadmap).
- **Long-running servers.** A built program does not yet free some memory, such as reference cycles. A session-length server is fine; watch the memory of one that runs for days.
- **macOS.** Brevi is released for macOS, but programs built there are not yet tested in CI, and `--standalone` is refused there.
- **Windows** is a beta; its limits are in [Known issues](repo:docs/KNOWN_ISSUES.md).
- **Security.** The binary has the same access as any program you run; Brevi does not sandbox it. See [Security](https://github.com/ets-public/brevi/blob/main/SECURITY.md#where-brevi-stands).

If your server does not build, the error line names the line that stopped it. Please [open an issue](https://github.com/ets-public/brevi/issues) with that line: real servers that fail are how Brevi finds what to fix next.

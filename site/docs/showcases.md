# Showcases
<!-- description: Real Python programs built with Brevi against CPython running the same files: MCP servers, token tools and pure-Python ML, with every number reproducible. -->

Each showcase is an ordinary Python program, run two ways: built with `brevi build --release`, and run by CPython. Both sides get the same input. Output has to match byte for byte, or the showcase doesn't count. The numbers are medians of 10 runs. The programs and the script that times them are in [bench/showcase](repo:bench/showcase), and `python3 bench/showcase/run.py` reruns them all.

<!-- showcases:start -->

| Showcase | What is timed | Brevi | CPython 3.13 | Faster | Same output | Binary | Needs Python |
| --- | --- | --- | --- | --- | --- | --- | --- |
| [SQLite MCP server](repo:bench/showcase/sqlite_mcp.py) | Start to first tool call answered | **29 ms** | 614 ms | **21x** | yes | 16.1 MB | no |
| [MCP time server](#the-official-mcp-time-server) (official, unchanged) | Start to first tool call answered | **29 ms** | 650 ms | **22x** | yes | 17.5 MB | no |

Measured 8 November 2026 with Brevi 0.25.0 (main) on Linux x86_64 with 4 cores. The data is [bench/showcase/results.json](repo:bench/showcase/results.json).

<!-- showcases:end -->

## SQLite MCP server

A notes server on the official [`mcp` Python SDK](https://github.com/modelcontextprotocol/python-sdk) with three tools: `add_note`, `search_notes` and a read-only `query`. The file is plain Python that CPython runs unchanged. Brevi builds it, SDK and SQLite included, into one 16 MB binary that does not link libpython. An agent client starts an MCP server for each session, so start-up time is the time a user waits.

```bash
brevi build bench/showcase/sqlite_mcp.py --release -o notes-mcp
claude mcp add notes -- ./notes-mcp ~/notes.db
```

The timed session is what a client sends: `initialize`, `tools/list`, then three tool calls, each sent once the reply before it arrives.

## The official MCP time server

[mcp-server-time](https://pypi.org/project/mcp-server-time/) is one of the reference servers the MCP project publishes, with 589k downloads a month. It tells a model the time in any IANA time zone and converts times between zones. Brevi builds its `server.py` as published, with no edits, into one 17.5 MB binary that does not link libpython. Brevi has its own `zoneinfo` and `tzlocal` for this: they read the system's time zone database and give the same answers as CPython's for every zone and every transition in it.

```bash
pip download mcp-server-time==2026.11.4 --no-deps
unzip -j mcp_server_time-2026.11.4-py3-none-any.whl mcp_server_time/server.py
curl -sO https://raw.githubusercontent.com/ets-public/brevi/main/bench/showcase/time_mcp.py
brevi build time_mcp.py --release -o time-mcp
claude mcp add time -- ./time-mcp
```

The timed session sets `TZ=Europe/Paris`, lists the tools, then makes eight tool calls: two conversions, an unknown zone, a zone in the wrong case, a bad time, a tool that does not exist (a JSON-RPC error from the server's own middleware), and a conversion from the local zone. [time_mcp.py](repo:bench/showcase/time_mcp.py) is the package's `mcp-server-time` command: it reads `--local-timezone` and runs `serve`.

## Next showcases

These are being made to build natively now. Each gets a row in the table above once its output matches CPython:

- **Token counting** in the style of [ttok](https://github.com/simonw/ttok) and [files-to-prompt](https://github.com/simonw/files-to-prompt): exact tiktoken counts from one binary with no Python installed.
- **Karpathy's [micrograd](https://github.com/karpathy/micrograd) and [microgpt](https://gist.github.com/karpathy/8627fe009c40f57531cb18360106ce95)**: an autograd engine and a GPT trained in pure Python. Today Brevi runs this class-heavy code slower than CPython, and the speed work is under way.

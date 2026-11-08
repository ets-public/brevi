# Showcases
<!-- description: Real Python programs built with Brevi against CPython running the same files: MCP servers, token tools and pure-Python ML, with every number reproducible. -->

Each showcase is an ordinary Python program, run two ways: built with `brevi build --release`, and run by CPython. Both sides get the same input. Output has to match byte for byte, or the showcase doesn't count. The numbers are medians of 10 runs. The programs and the script that times them are in [bench/showcase](repo:bench/showcase), and `python3 bench/showcase/run.py` reruns them all.

<!-- showcases:start -->

| Showcase | What is timed | Brevi | CPython 3.13 | Faster | Same output | Binary | Needs Python |
| --- | --- | --- | --- | --- | --- | --- | --- |
| [SQLite MCP server](repo:bench/showcase/sqlite_mcp.py) | Start to first tool call answered | **29 ms** | 614 ms | **21x** | yes | 16.1 MB | no |

Measured 8 November 2026 with Brevi 0.25.0 (main) on Linux x86_64 with 4 cores. The data is [bench/showcase/results.json](repo:bench/showcase/results.json).

<!-- showcases:end -->

## SQLite MCP server

A notes server on the official [`mcp` Python SDK](https://github.com/modelcontextprotocol/python-sdk) with three tools: `add_note`, `search_notes` and a read-only `query`. The file is plain Python that CPython runs unchanged. Brevi builds it, SDK and SQLite included, into one 16 MB binary that does not link libpython. An agent client starts an MCP server for each session, so start-up time is the time a user waits.

```bash
brevi build bench/showcase/sqlite_mcp.py --release -o notes-mcp
claude mcp add notes -- ./notes-mcp ~/notes.db
```

The timed session is what a client sends: `initialize`, `tools/list`, then three tool calls, each sent once the reply before it arrives.

## Next showcases

These are being made to build natively now. Each gets a row in the table above once its output matches CPython:

- **The official MCP time server** ([mcp-server-time](https://pypi.org/project/mcp-server-time/), 589k downloads a month), built from the unchanged upstream file. It needs Brevi's own `zoneinfo`.
- **Token counting** in the style of [ttok](https://github.com/simonw/ttok) and [files-to-prompt](https://github.com/simonw/files-to-prompt): exact tiktoken counts from one binary with no Python installed.
- **Karpathy's [micrograd](https://github.com/karpathy/micrograd) and [microgpt](https://gist.github.com/karpathy/8627fe009c40f57531cb18360106ce95)**: an autograd engine and a GPT trained in pure Python. Today Brevi runs this class-heavy code slower than CPython, and the speed work is under way.

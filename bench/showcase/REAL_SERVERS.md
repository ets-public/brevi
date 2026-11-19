# Ten popular Python MCP servers, built unchanged with Brevi

Run 2026-11-19 on Brevi 0.28.3 (main at 20ab11d), in a cloud container (4 vCPU Xeon 2.1 GHz, 15 GB RAM), CPython 3.13.18, uv 0.11.32.
Every server's code ran inside `tools/modelbox.py` (no network, no credentials), never on a developer machine.
Raw data, harness and logs: project files `real-servers-1119/` (results.json, specs.py, bench_one.py, flat.py, *.log).

**Result in one line: none of the ten builds natively. All ten build only as CPython-bridge binaries, which need Python
on the machine and start no faster than CPython (1.00x to 1.16x CPython's time to first tool call, over the builds whose session matched).** The 35 ms time-server
showcase does not carry over to the current release of that same server (2026.11.18; the showcase pins 2026.11.4).

## 1. How the ten were picked

- Source: PyPI downloads in the last month, from pypistats.org (`/api/packages/<name>/recent`), read 2026-11-19 09:55Z.
- Candidates: about 100 known Python MCP server packages on PyPI were ranked by those downloads (list and counts in
  `pypi_rank.txt`). No candidate was dropped for what Brevi supports. This is a seeded list, not a full PyPI crawl, so
  a popular server missing from it would be missed.
- Two of the top 11 were left out, both for reasons outside Brevi:
  - **mcp-grafana** (#5, 489,630) ships a Go binary in its wheel, so it is not Python.
  - **postgres-mcp** (#4, 609,014) cannot be installed from PyPI on CPython either, at 0.3.0, 0.2.1 or 0.2.0, because
    it pins `pglast==7.2.0` and PyPI has no such build.
- #11, workspace-mcp, takes their place. Versions are the latest on PyPI that day, installed from wheels only into one
  venv per server.

## 2. Outcome per server

How each was built:
- **installed**: `srv.py` is `from <package> import main; main()` against the venv, with the package unchanged in
  site-packages. This is what a user with `pip install` has.
- **source**: the same, with the package folder copied beside `srv.py`, so Brevi compiles the server's own code.
- Brevi's path is the `{"path": ...}` line it prints.

**Needs Python** means the binary loads libpython and the build venv's site-packages at run time. A pybridge binary
does not link libpython (`ldd` shows none), so `ldd` cannot tell you this. `--standalone` builds carry their own
CPython instead (see section 3).

**Blocker** comes from building a *flattened* copy (package files side by side, relative imports turned into plain
ones, by `flat.py`). That is the only way to see Brevi's reason, because the package build gives none. Rows marked
"not flattened" list the imports Brevi has no copy of, which was not confirmed by a build.

| # | Server | PyPI downloads (month) | Outcome (installed) | Path (source) | Blocker | Needs Python |
|---|---|---:|---|---|---|---|
| 1 | mcp-atlassian 0.24.6 | 1,622,134 | bridged | pybridge | not flattened (123 files in subpackages); imports with no Brevi copy: fastmcp, atlassian, bs4, click, keyring, starlette and 14 more | yes |
| 2 | mcp-server-fetch 2026.11.18 | 1,025,020 | bridged | pybridge | typed: argparse Namespace attribute `.allow_private_ips` aliasing refusal; dynamic: asyncio use beyond Brevi's asyncio | yes |
| 3 | mcp-server-time 2026.11.18 | 738,381 | bridged | pybridge | typed: false aliasing refusal on `INVALID_PARAMS` (an int constant from mcp.types); dynamic: asyncio use beyond Brevi's asyncio | yes |
| 4 | mcp-server-git 2026.11.18 | 423,512 | bridged | pybridge | typed: aliasing refusal on `.commit`; dynamic: asyncio use beyond Brevi's asyncio | yes |
| 5 | mcp-proxy 0.15.0 | 417,450 | bridged | dynamic | typed: argparse `.allowed_host` aliasing refusal; dynamic: asyncio use beyond Brevi's asyncio; also mcp.client.* (no Brevi copy) | yes |
| 6 | awslabs.aws-api-mcp-server 1.6.1 | 409,609 | bridged | dynamic | not flattened (40 files); imports with no Brevi copy: fastmcp, boto3, botocore, awscli, loguru, lxml | yes |
| 7 | awslabs.aws-documentation-mcp-server 1.2.6 | 362,788 | bridged | dynamic | typed: decorator on `read_documentation` dropped; dynamic: async code over CPython libraries (bs4, loguru, markdownify) | yes |
| 8 | chroma-mcp 0.2.6 | 357,850 | bridged | pybridge | imports mcp.server.fastmcp (no Brevi copy); typed: aliasing refusal on `settings`; dynamic: coroutine handed to CPython libraries (chromadb) | yes |
| 9 | mcp-clickhouse 0.7.0 | 349,058 | bridged | dynamic | typed: argparse `.bind_host` aliasing refusal; dynamic: asyncio use beyond Brevi's asyncio; imports fastmcp (no Brevi copy) | yes |
| 10 | workspace-mcp 2.5.0 | 319,896 | bridged | not tried (entry is a top-level main.py) | not flattened (top-level main.py plus auth/core/gmail packages); imports fastmcp, fastapi, google-api-python-client (no Brevi copy) | yes |

Paths: `pybridge` means the typed native code with the whole server package in CPython. `dynamic` means Brevi's
dynamic build, which also handed the whole package to CPython (`"py": ["<package>"]`). Four servers took the dynamic
path, at a build cost of about 2 minutes and 1.55 GB of RAM, and gained no native code for it.

## 3. Build cost, size and start-up

Builds ran with `brevi build srv.py --release --no-cache` (plus `--standalone` for the third figure). Brevi's runtime
was already on disk from `cargo build`, so this is a cold result cache, not a first install.

Build peak RAM is the largest single process (`RUSAGE_CHILDREN` max RSS).

| Server | Build s (installed / source / standalone) | Build peak MB (same order) | Binary MB (same order) |
|---|---|---|---|
| mcp-atlassian | 2.9 / 2.7 / 9.5 | 294 / 293 / 297 | 0.7 / 0.7 / 41.2 |
| mcp-server-fetch | 2.8 / 2.8 / 8.0 | 293 / 295 / 298 | 0.7 / 0.7 / 30.7 |
| mcp-server-time | 2.7 / 2.7 / 7.6 | 293 / 293 / 302 | 0.7 / 0.7 / 23.9 |
| mcp-server-git | 2.7 / 2.8 / 7.2 | 294 / 293 / 301 | 0.7 / 0.7 / 23.9 |
| mcp-proxy | 2.8 / 122.0 / 6.3 | 293 / 1556 / 302 | 0.7 / 10.4 / 23.6 |
| awslabs.aws-api-mcp-server | 2.8 / 117.9 / 11.6 | 293 / 1559 / 299 | 0.7 / 10.4 / 65.7 |
| awslabs.aws-documentation-mcp-server | 2.8 / 116.0 / 5.5 | 296 / 1552 / 296 | 0.7 / 10.4 / 19.1 |
| chroma-mcp | 2.7 / 2.7 / 21.7 | 294 / 294 / 299 | 0.7 / 0.7 / 124.0 |
| mcp-clickhouse | 2.8 / 118.6 / 8.7 | 294 / 1556 / 298 | 0.7 / 10.4 / 32.1 |
| workspace-mcp | 2.7 / n/a / 12.6 | 295 / n/a / 299 | 0.7 / n/a / 53.1 |

The 0.7 MB pybridge binary is small only because the server stays in the venv it was built against.

**First tool call** is the median of 5 runs, timed from process start to the first `tools/call` reply in the session
below. Parity compares every reply in that session against CPython. RSS is the server's peak.

| Server | CPython ms | uvx ms | Brevi installed ms | Brevi source ms | Brevi standalone ms | Peak RSS MB (CPython / installed / standalone) | Parity, installed and source | Parity, standalone |
|---|---:|---:|---:|---:|---:|---|---|---|
| mcp-atlassian | 1,678 | 1,837 | 1,732 | 1,818 | n/a | 134.1 / 135.0 / 82.4 | same | BROKEN: exits at start, `FastMCP server support is not installed` (bundle misses fastmcp) |
| mcp-server-fetch | 789 | 852 | 802 | 834 | 790 | 89.2 / 90.4 / 103.8 | same | same |
| mcp-server-time | 642 | 705 | 681 | 666 | 703 | 69.2 / 71.9 / 84.8 | same | same |
| mcp-server-git | 672 | 749 | 750 | 762 | 777 | 71.4 / 74.5 / 85.7 | same | same |
| mcp-proxy | n/a | n/a | n/a | n/a | n/a | 59.8 / 60.2 / 70.8 | same failure | same failure |
| awslabs.aws-api-mcp-server | n/a | n/a | n/a | n/a | n/a | 119.3 / 121.6 / 88.1 | same failure | same failure |
| awslabs.aws-documentation-mcp-server | 745 | 815 | 834 | 840 | n/a | 81.2 / 82.7 / 67.8 | same | BROKEN: exits at start, `No module named 'cryptography'` |
| chroma-mcp | n/a | n/a | n/a | n/a | n/a | 89.2 / 92.8 / 80.4 | same failure | same failure |
| mcp-clickhouse | 1,168 | 1,193 | 1,222 | 1,236 | broken | 102.7 / 103.7 / 113.6 | same | BROKEN: tools/list errors `No module named 'key_value'` |
| workspace-mcp | 5,168 | 5,161 | 5,276 | n/a | broken | 142.8 / 133.9 / 141.7 | same up to OAuth scope order (Python set order; CPython vs uvx differ the same way) | BROKEN: tools/list errors `MemoryStore requires py-key-value-aio[memory]` (bundle misses key_value) |


- **uvx** is `uvx --offline --from <pkg>==<ver> <script>` with a warm uv cache.
- **Standalone** times are warm runs: the bundled CPython unpacks once into `~/.cache/brevi/py`, and that first run
  is included in the 5-run median. A standalone build whose session broke shows no time.
- **"same failure"** means CPython itself cannot run a session offline, and Brevi fails the same way:
  - mcp-proxy needs a remote SSE server.
  - aws-api fetches an AWS service list at start-up.
  - chroma-mcp 0.2.6 fails to import, because its chromadb does not work with today's pydantic.
  
  These rows have no start time.

## 4. The session

Every server got the same lockstep session of at least 7 requests:

1. `initialize`, then `notifications/initialized`.
2. `tools/list`, `prompts/list` and `resources/list`.
3. 2 to 5 real tool calls, including at least one error path.
4. A call to an unknown tool.

The calls per server:
- **time**: 4 `convert_time` calls (two good, a bad zone, a bad time).
- **fetch**: a fetch of a private IP (refused), a bad URL and a missing argument.
- **git**: status, log, unstaged diff and branch on a fixture repo, plus `git_show` of a bad ref.
- **aws-docs**: `read_documentation` on an unreachable URL and on an unsupported domain.
- **chroma**: list, create, list, count, and info on a missing collection.
- **clickhouse**: `list_databases` and `run_query` against an unreachable host.
- **workspace**: two Gmail calls that need authentication.
- **atlassian**: lists no tools without credentials, so it gets only the unknown-tool call.

Calls that need a live service were checked on their error replies, which were compared byte for byte. The exact
messages are in `specs.py`.

## 5. Bugs found (each is a lane in the handoff)

1. **No unchanged server builds natively.** Two refusals recur:
   - The typed build's aliasing refusal hits ordinary `argparse` and settings code (`args.allow_private_ips`,
     `.bind_host`, `.allowed_host`). It also hits `INVALID_PARAMS`, an int constant from `mcp.types`, which looks like
     a wrong diagnosis.
   - The dynamic build reports "asyncio beyond Brevi's own asyncio", without saying which asyncio API.
2. **The showcase regressed upstream.** mcp-server-time 2026.11.18 no longer builds natively. 2026.11.4 does, and that
   is the version the 35 ms showcase uses.
3. **The package build hides its reason.** A server built as a package reports only `"<pkg> run in CPython"`. The
   dynamic build's refusal is known at src/main.rs:855-858 and dropped at :730, and even `BREVI_DYN_WHY=1` does not
   print it.
4. **The dynamic path costs much and gains nothing.** It takes about 2 minutes and 1.55 GB of RAM, then hands the
   whole package to CPython anyway.
5. **`--standalone` misses lazily imported packages.** It breaks 4 of 10 servers at run time: fastmcp (atlassian),
   cryptography (aws-docs), and key_value (clickhouse, workspace).
6. **`--native` builds a pybridge binary and exits 0.** The docs say it "never falls back to CPython".
7. **`brevi build --release srv.py` fails** with `{"e":"io","at":"--release",...}`: the flag has to come after the
   file.
8. **No Brevi copy** of `mcp.server.fastmcp` (chroma-mcp), `mcp.client.*` (mcp-proxy) or the separate `fastmcp`
   package (atlassian, aws-api, clickhouse, workspace).
9. **`ldd` cannot show that a pybridge binary needs Python.** bench/showcase/run.py's `links_python` reports False
   for these.

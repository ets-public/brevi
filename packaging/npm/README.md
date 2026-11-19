# Brevi: turn your Python MCP server or CLI into one fast binary

Brevi compiles the Python you already have, imports included, through Rust into one native file. The machine that runs it needs no Python, no virtualenv and no packages. The official `mcp-server-time` 2026.11.4, built unchanged, answered its first tool call in 35 ms where CPython 3.13 took 730 ms, from one 19.2 MB file that links no libpython (Brevi 0.26.0, Linux x86-64, 4-core Intel Xeon, median of 10 runs; [Showcases](https://brevi-lang.org/docs/showcases)). The win is start-up and import time, not throughput: the dynamic build most servers take runs at about CPython's speed once started (median 0.85x on our compute benchmarks). Most real servers do not get that today: the current `mcp-server-time` (2026.11.18) and, in our first real-server test (19 November 2026), all of the 10 most downloaded MCP servers on PyPI fall back to the CPython bridge, which needs Python's shared library on the run machine and loses the fast start. When a line cannot be built, the build says which one in one JSON line.

```
npm install -g brevi     # or run it without installing: npx brevi mcp
```

The first run downloads the prebuilt `brevi` for your platform from the matching [GitHub release](https://github.com/ets-public/brevi/releases) into `~/.cache/brevi/VERSION`.

```
$ brevi build server.py --release -o my-server   # one native file to ship
$ claude mcp add mine -- /full/path/to/my-server # register it
$ brevi verify server.py --release --input session.jsonl   # check it against CPython
```

A sure mistake is reported before anything runs, as one line with a fix:

```
{"line":8,"msg":"name 'totl' is not defined","fix":"did you mean total?"}
```

Brevi is needed only on the machine that builds. With no Rust there, the first build fetches Brevi's pinned Rust release once (about 300 MB), checked against SHA-256 sums built into Brevi (Linux and macOS from v0.28.2, Windows and Linux ARM64 from v0.28.3).

## Node SDK

From v0.27.0 the package is also a library, for harnesses that run the code a model writes (`npm install brevi`):

```js
import { run, verify, handle, TOOL, Session } from "brevi";

const r = await run("print(sum(range(10)))");   // sandboxed: no files, network or CPython unless granted
r.stdout, r.exitCode, r.path                     // "45\n", 0, "typed"
await run(code, { stdin: "3 4\n", limits: { time: 5, mem: 256 }, allowRead: ["/data"] });
await run(code, { cases: [{ stdin: "1\n", expect: "2" }] });  // r.ok; r.text names the first differing line
await verify(code, { inputs: ["1\n", "2\n"] });  // r.certificate.verify === "identical"
const reply = await handle(toolUse.input);        // run a model's run_python call; TOOL is its definition
```

A `Result` has `stdout`, `stderr`, `exitCode`, `path`, `limit`, `certificate`, `ok`, `ms` and `forModel()`. Grants and limits (`allow`, `allowPy`, `allowNet`, `allowRead`, `allowWrite`, `limits`, `timeout`, `sandbox`) come only from your code, per call or as `new Session({...})` defaults; `TOOL` gives the model no field for them. Each session is one `brevi mcp` process, and its calls are taken in turn. TypeScript types are included, and `require("brevi")` works too.

- **Before you ship:** imports Brevi does not build itself (pandas, yaml, ...) run in CPython inside the binary and need Python's shared library on the run machine, or `--standalone` (Linux and Windows). Linux binaries link glibc, so build on the oldest Linux you target. macOS builds are not yet tested in CI; Windows is a beta. A built program does not yet free some memory, such as reference cycles, so watch a server that runs for days. [Ship an MCP server or CLI](https://brevi-lang.org/docs/bundle-mcp)
- **For coding agents:** add the MCP server as the command `brevi` with the argument `mcp` (Claude Code: `claude mcp add brevi -- brevi mcp`). Its tools sandbox by default. [For AI agents](https://brevi-lang.org/docs/agents)
- **Security:** No external security audit yet. The sandbox is a policy layer, not an isolation boundary. Run untrusted code in a container or VM. `--allow-py` and `--allow native` are full trust. [Where Brevi stands](https://github.com/ets-public/brevi/blob/main/SECURITY.md#where-brevi-stands)
- **Same answers as CPython:** every saved agent program and large test program matches CPython's output and exit status. Imports of installed packages (numpy, pandas, requests, ...) work; most numpy code builds natively.
- **Public preview:** Brevi works today and is tested against CPython on every change, but expect the CLI to change between releases.

[Docs](https://brevi-lang.org/docs/) · [Source](https://github.com/ets-public/brevi) · [Changelog](https://github.com/ets-public/brevi/blob/main/CHANGELOG.md) · [Issues](https://github.com/ets-public/brevi/issues) · Apache-2.0

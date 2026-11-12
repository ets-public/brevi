# Brevi: fast, safe native builds of AI-written Python

Brevi is a compiler built for AI coding agents. The agent writes the plain Python it already writes best; Brevi builds it through Rust into a native binary that runs 1.6x to 37.9x faster than CPython (median 9.3x) on our 12 compute benchmarks with the default build, and aims to print exactly what CPython prints (58% of CPython 3.13's own tests match today; 14% still build and differ). Anything it cannot build comes back as one short JSON line the agent can act on.

```
npm install -g brevi     # or run it without installing: npx brevi mcp
```

The first run downloads the prebuilt `brevi` for your platform from the matching [GitHub release](https://github.com/ets-public/brevi/releases) into `~/.cache/brevi/VERSION`.

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

- **For agents:** add the MCP server as the command `brevi` with the argument `mcp` (Claude Code: `claude mcp add brevi -- brevi mcp`). Its tools sandbox by default. [For AI agents](https://brevi-lang.org/docs/agents)
- **Same answers as CPython:** every saved agent program and large test program matches CPython's output and exit status. Imports of installed packages (numpy, pandas, requests, ...) work; most numpy code builds natively.
- **Public preview:** Brevi works today and is tested against CPython on every change, but expect the CLI to change between releases.

[Docs](https://brevi-lang.org/docs/) · [Source](https://github.com/ets-public/brevi) · [Changelog](https://github.com/ets-public/brevi/blob/main/CHANGELOG.md) · [Issues](https://github.com/ets-public/brevi/issues) · Apache-2.0

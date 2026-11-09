# Brevi: fast, safe native builds of AI-written Python

Brevi is a compiler built for AI coding agents. The agent writes the plain Python it already writes best; Brevi builds it through Rust into a native binary that runs 1.7x to 39.5x faster than CPython (median 8.5x) on our 12 compute benchmarks with the default build, and aims to print exactly what CPython prints (58% of CPython 3.13's own tests match today; 14% still build and differ). Anything it cannot build comes back as one short JSON line the agent can act on.

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

## Python SDK

From v0.27.0 the same package is also a Python library, for harnesses and agent platforms that run the code a model writes:

```python
import brevi

r = brevi.run("print(sum(range(10)))")   # sandboxed: no files, network or CPython unless you grant them
r.stdout, r.exit_code, r.path            # ('45\n', 0, 'typed')
r = brevi.run(code, stdin="3 4\n", limits={"time": 5, "mem": 256}, allow_read=["/data"])
r = brevi.run(code, cases=[{"stdin": "1\n", "expect": "2"}])     # r.ok; r.text names the first differing line
r = brevi.verify(code, inputs=["1\n", "2\n"])  # r.certificate["verify"] == "identical": answers as CPython does
reply = brevi.handle(tool_call.input)           # run a model's run_python call; brevi.TOOL is its definition
```

A `Result` has `stdout`, `stderr`, `exit_code`, `path` (`typed`, `dynamic`, `pybridge`, `cpython`, or `None` when it did not build), `limit` (the limit that stopped it), `certificate`, `ok`, `ms` and `for_model()`. Grants and limits come only from your code: `allow`, `allow_py`, `allow_net`, `allow_read`, `allow_write`, `limits`, `timeout` and `sandbox` on each call or as `brevi.Session(...)` defaults; `TOOL` gives the model no field for them, and an `allow` line in the program grants nothing. `brevi.AsyncSession` has the same calls for asyncio. Each session is one `brevi mcp` process, so the build cache stays warm between calls.

`brevi-python` is a drop-in for `python3`: `brevi-python script.py`, `-c CODE` and a script on stdin build natively when Brevi can and run with CPython otherwise, with CPython's output and exit status and nothing from Brevi (`BREVI_SHIM_VERBOSE=1` shows which path ran). Other flags, `-m` and the REPL go straight to CPython, as does a program that reads what differs between the two (`sys.executable`, `sys.argv[0]`, argparse). `BREVI_SHIM=0` turns it off.

[Docs](https://brevi-lang.org/docs/) · [Source](https://github.com/ets-public/brevi) · [Changelog](https://github.com/ets-public/brevi/blob/main/CHANGELOG.md) · [Issues](https://github.com/ets-public/brevi/issues) · Apache-2.0

<!-- Lets the MCP registry confirm this package belongs to the server io.github.ets-public/brevi. -->
mcp-name: io.github.ets-public/brevi

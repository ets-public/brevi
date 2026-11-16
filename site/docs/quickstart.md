# Quickstart for agents
<!-- description: A few commands from install to a verified run: write main.py, read Brevi's one-line JSON error, fix the line, and get a certificate that the native run matches CPython. -->

Brevi is the verified native runtime for the Python your agent writes. The agent writes plain Python; Brevi compiles
it to a native binary, checks the run against CPython 3.13, and says so in one line of JSON whenever it cannot build
or cannot check a program. This page is the whole loop, as your harness will drive it.

## 1. Install

<!-- install-line: the one place this page names the install command; swap it when the install changes -->
With [uv](https://docs.astral.sh/uv/) installed, one command installs Brevi and runs a program (step 2 writes `main.py`):

```
uvx --python 3.13 brevi run --verify main.py
```

It downloads brevi and CPython 3.13 (the version `--verify` checks against), and on the first build the Rust release
Brevi is tested with, plus zig to link with when the machine has no C linker, each checked against a SHA-256 compiled
into brevi. That is about 300 MB once and a minute or two; later builds take seconds. Fetching uses `curl` and `tar`
(macOS and Windows have both; a bare Linux container needs `curl`, `ca-certificates` and `xz-utils`).

Nothing else is needed on Linux x86-64 and macOS, from v0.28.2. Linux ARM64 and Windows get the same from the next
release; until then, install Rust with [rustup](https://rustup.rs) first (on Windows it also asks for the Microsoft
C++ build tools), then run the same command.

The commands below say `brevi`: write `uvx --python 3.13 brevi` instead, or keep it on your PATH with
`uv tool install --python 3.13 brevi`. The install script, pip, npm, Docker and the release archives:
[Install](/docs/install).
<!-- /install-line -->

`--verify` compares against CPython 3.13 and no other version. Brevi finds one as `python3`, `python3.13` or
`python` on PATH, through `uv` (so the CPython `uvx --python 3.13` fetched counts), or named by `BREVI_PYTHON`.

```
$ brevi --version
brevi __FULLVERSION__
```

## 2. The agent writes main.py

Plain Python, nothing Brevi-specific. Here, a first draft with a typo on the last line:

```python
import sys

def primes_below(n):
    sieve = [True] * n
    sieve[0:2] = [False, False]
    for i in range(2, int(n ** 0.5) + 1):
        if sieve[i]:
            for j in range(i * i, n, i):
                sieve[j] = False
    return [i for i, p in enumerate(sieve) if p]

n = int(sys.stdin.read())
ps = primes_below(n)
total = sum(ps)
print(len(ps), "primes below", n)
print("sum", totl)
```

## 3. Check it, read the JSON error

```
$ brevi check main.py
{"line":16,"msg":"name 'totl' is not defined","fix":"did you mean total?"}
```

Brevi checks the whole program before anything runs. A sure mistake comes back as one line, with the line number and
a fix; the MCP `check` and `run` tools answer with the same line. Feed it back to the model: it is the whole error,
with no traceback to read.

If the agent skips the check and goes straight to `brevi run --verify`, Brevi cannot build the file, so it runs it
with CPython and says so first, naming the line that stopped the build; CPython then raises the `NameError` where it
reaches line 16:

```
$ echo 1000 | brevi run --verify main.py
{"path":"cpython","line":16,"why":"Brevi could not build it: name 'totl' is not defined","by":"python3"}
168 primes below 1000
NameError: name 'totl' is not defined. Did you mean: 'total'? (main.py line 16)
```

## 4. Fix the line, verify

The agent changes line 16 to `print("sum", total)` and runs again:

```
$ echo 1000 | brevi run --verify main.py
168 primes below 1000
sum 76127
{"verify":"identical","file":"main.py","sha256":"…","inputs":["sha256:…"],"args":[],"path":"typed","exit":0,"stdout_same":true,"stderr_same":true,"brevi_ms":5,"cpython_ms":13,"speedup":2.6,"brevi":"0.28.2","cpython":"3.13.18"}
```

The answer is on stdout and the certificate on stderr: the native build and CPython 3.13 gave the same stdout, exit
code and stderr on this input. Log it; it carries the hashes of the file and the input.

What `--verify` does when it cannot certify a run:

| Verdict line on stderr | Meaning | What you get |
|---|---|---|
| `{"verify":"identical",...}` | Native and CPython agree on this input | The native answer |
| `{"verify":"differs","on":"stdout","line":N,"brevi":...,"cpython":...,...}` | They disagree: almost always a Brevi bug (output CPython itself varies, such as a printed set of strings, can differ too; `--strict` runs CPython twice and reports `unstable`) | **CPython's** answer and exit code; logged for `brevi report` |
| `{"verify":"refused","why":"reads the clock",...}` | Running it twice could change the answer or repeat a side effect (clock, unseeded random, environment, file writes, network) | The program's own answer, marked unchecked |
| `{"verify":"timeout",...}` | CPython had not finished well after the native run, or hit the limit | The native answer, marked unverified |
| `{"verify":"refused","why":"Brevi answers as CPython 3.13 does, so it runs only CPython 3.13, and ...",...}` | No CPython 3.13 was found | The program's own answer, marked unchecked |
| `{"path":"cpython",...}` | Brevi could not build it natively | CPython ran it; the line says which line and function stopped the build |

`identical`, `differs` and `timeout` come after the program's output; `refused` and `{"path":"cpython"}` come before it.

So your harness never gets an unchecked answer presented as checked. `--verify` costs one CPython run per call;
on heavy programs, verify on the test inputs and then run with plain `brevi run` or a built binary
(`brevi build main.py -o app --release`).

## 5. Wire it into the harness

- **MCP:** `brevi mcp` serves `check`, `run`, `verify` and the edit tools over stdio (Claude Code:
  `claude mcp add brevi -- brevi mcp`). `run` checks by default; under the sandbox, which is the default, the check
  is against Brevi's dynamic build, because CPython cannot be held to the sandbox (`"against":"dynamic"`). Only programs on the typed build are compared this way; others get `{"verify":"refused","against":"dynamic",...}`. A server
  started with `--trust-calls` can take `sandbox: false` and check against CPython. [For AI agents](/docs/agents)
- **Python SDK:** `pip install brevi`, then `brevi.run(code, stdin=...)` and `brevi.verify(code, inputs=[...])`.
- **A runnable demo** of this whole loop, MCP included, with the timings it measured:
  [examples/agent-loop](__GITHUB__/tree/main/examples/agent-loop).

## What it is, and is not

- It checks **your program on your inputs**. That is the guarantee; it is not a claim that all of Python works.
  How much of CPython's own test suite Brevi matches is in [What works, and limits](/docs/compatibility).
- It is **not cheaper in model tokens** on ordinary tasks: in our release test the API bill came out within 8% of
  plain CPython (Sonnet 1.08x, Haiku 1.00x) ([Benchmarks](/docs/benchmarks)). What you get is a native binary, one-line errors and a
  per-run check.
- The sandbox is a policy layer, not an isolation boundary: run untrusted code in a container or VM as well
  ([threat model](__GITHUB__/blob/main/docs/THREAT_MODEL.md)).

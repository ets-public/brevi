# The agent loop, end to end

One continuous run of what Brevi is for: an agent writes plain Python, Brevi compiles it, `--verify` checks the
native answer against CPython and refuses visibly what it cannot check, and the program runs natively or through
the MCP server.

```
bash examples/agent-loop/demo.sh
```

Needs `brevi` on PATH and CPython 3.13 (`python3`, or `BREVI_PYTHON`) for the comparison. Every step checks its own
result, so the script is also the smoke test the [install tests](../../.github/workflows/install-test.yml) run (by hand and on each
release) on a fresh Ubuntu container and GitHub's macOS and Windows runners.

| File | What it is |
|---|---|
| `draft.py` | The agent's first draft: count and sum the primes below N, with a typo on line 20 |
| `main.py` | The same program after the agent fixes the one line Brevi named |
| `mcp_run.py` | A 30-line harness: sends a program to `brevi mcp` over stdio and prints the reply (standard library only) |
| `demo.sh` | Runs the loop and checks each step |

## What it printed

Brevi 0.28.1 on a 4-core Linux container, CPython 3.13.18. Timings are from that machine; yours will differ, and the
script prints its own.

```
== 1. The agent's first draft has a typo on line 20
$ brevi check draft.py
{"line":20,"msg":"name 'totl' is not defined","fix":"did you mean total?"}

== 2. The agent fixes line 20 and runs with --verify
$ echo 1000 | brevi run --verify main.py
168 primes below 1000
sum 76127
{"verify":"identical","file":"main.py","sha256":"235063dd…","inputs":["sha256:83c02ac2…"],"args":[],"path":"typed","exit":0,"stdout_same":true,"stderr_same":true,"brevi_ms":2,"cpython_ms":18,"speedup":9.0,"brevi":"0.28.1","cpython":"3.13.18"}

== 3. A program that reads the clock cannot be checked by running it twice: refused, and said so
$ brevi run --verify clock.py
{"verify":"refused","why":"reads the clock","msg":"not checked against CPython: a second run could answer differently or repeat what it does; run as plain brevi run"}
clock read: True

== 4. The same fix through the MCP server (sandboxed by default, verify on by default)
$ python3 mcp_run.py main.py 1000
168 primes below 1000
sum 76127
{"verify":"identical","against":"dynamic","typed_ms":1,"dynamic_ms":3}

== 5. Latency against CPython, on N = 20,000,000 (timed on this machine)
1270607 primes below 20000000
sum 12272577818052
native 396 ms, CPython 2327 ms: 5.9x (best of 3, same output)
```

## Reading it

- **Step 1** is the whole error an agent reads: one line, the line number, and a fix. No traceback, and nothing ran.
- **Step 2** is the answer plus a certificate on stderr: the native build and CPython 3.13 gave the same stdout,
  exit code and stderr on this input, with hashes of the file and input a harness can log.
- **Step 3** is the refusal. A program that reads the clock, unseeded random or the environment, writes files or
  uses the network cannot be compared by running it twice, so `--verify` says so in one line and the result is
  marked unchecked. When the native answer and CPython's **differ**, `--verify` prints CPython's answer, exits with
  CPython's code and puts the first differing line on stderr:
  `{"verify":"differs","on":"stdout","line":3,"brevi":"...","cpython":"...",...}` (logged for `brevi report`). Brevi has no known
  wrong answer on its scored CPython tests, so this demo cannot show a real one; `tests/verify/fakepy.sh` is the
  stand-in CPython the test suite uses to check that path.
- **Step 4** is the same loop through MCP. Under the sandbox, which the MCP server uses by default, CPython cannot be
  held to the sandbox, so the check is against Brevi's own dynamic build (`"against":"dynamic"`), a weaker check
  than CPython, and only for programs on the typed build; others get `{"verify":"refused","against":"dynamic",...}`. A server started with `brevi mcp --trust-calls` can take `sandbox: false` and check against CPython.
- **Step 5** is speed, on the typed build. Not every program takes it: a program whose values change type gets
  the dynamic build, which runs at about CPython's speed. Measured figures for both builds are in
  [Benchmarks](https://brevi-lang.org/docs/benchmarks).

## Cost and tokens

`--verify` costs one CPython run per call (18 ms here); the native side of a cached run is a few milliseconds.
On model cost, the release test (Claude Sonnet and Haiku, 24 tasks in plain Python, checked once by CPython and
once by Brevi) measured Brevi at 1.08x CPython's API bill for Sonnet and 1.00x for Haiku at v0.28.0: Brevi does
not save model cost on ordinary tasks. What it adds is the native binary, the one-line errors and the per-run
check. Method and logs: [bench/README.md](../../bench/README.md).

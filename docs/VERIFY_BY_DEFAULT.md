# Design: verify by default

Status: built 9 November 2026 (PR #283, ships in v0.27.0): `brevi run --verify`, `brevi report`, the MCP `run`
default and the cases rule as below; the release test logs `verify_differs` per pyb task (`bench/e2e.py`). Design: 9 November 2026 (ROADMAP.md, WS1). Builds on `brevi verify` and the MCP `verify` tool, which exist
since v0.26.0 (`src/verify.rs`).

## The problem

Brevi matches CPython on 80.4% of the CPython conformance slice and gives a wrong answer on 8 tests (v0.27.0; 58.2% and 44 at v0.26.0). On main, from the next release: 93.1% and none. A harness that
makes Brevi its default Python takes on that risk for every program. `brevi verify` already proves, per program and
per input, that the native build answers as CPython does, but an agent has to know to call it, and nobody does. The
fix is to make the check part of the commands agents already run, at a cost they don't notice.

## What changes

### 1. `brevi run --verify FILE.py [ARGS]`

Runs the program once natively for the caller, and checks that run against CPython on the same stdin and args.

1. Build natively (strict, as `verify` does). If Brevi refuses, run CPython as `brevi run` does today, print the
   `{"path":"cpython",...}` line, and stop: there is nothing to verify.
2. Read stdin fully (it is needed twice), run the binary, and keep its stdout, stderr and exit code.
3. Run CPython once on the same stdin and args (twice with `--strict`, so unstable output is caught).
4. Compare stdout bytes, the exit code, and stderr with traceback frames removed (the rules of `verify.rs`).
5. **Same:** print the native run's output and exit with its code. The certificate line goes to stderr.
6. **Different:** print **CPython's** output and exit with CPython's code (the answer the program means), and put the
   first differing line on stderr: `{"verify":"differs","line":3,"brevi":"...","cpython":"..."}`. With `--strict`,
   print nothing from either run and exit 1 instead. A difference is logged for `brevi report` (below).

The caller always gets CPython's answer or a certified native one, never an unchecked native one. The price is a
CPython run per `run --verify`, which is why it is a flag for the CLI and a default only where inputs are small.

**Side effects.** A program that writes files, uses the network or reads the clock cannot be run twice safely.
`--verify` refuses (`{"verify":"refused","why":"writes files"}`, then a plain `brevi run`) when the build reports
any `write`, `net`, or `native` use, or the program reads `time`, `random` without a seed, `os.environ` or `sys.argv[0]`.
The file, network and native uses come from the check the sandbox already runs (`caps::check` with nothing
granted); the clock, unseeded random and environment reads are a small new scan over the same AST.

**Long programs.** CPython runs alongside the binary, not after it, with the same `--timeout`. If the native run
finishes and CPython has not after 10x the native time (at least 2 s), the run reports `{"verify":"timeout"}`,
returns the native answer, and marks it unverified on stderr. `--strict` waits for CPython instead.

### 2. MCP `run` with `cases`

When `run` gets `cases`, each case already has an `expect`, so the test is the oracle and CPython adds little. What
changes is that a native case that fails while CPython passes the same case is reported as a Brevi bug, not as the
model's bug:

```
case 2: differs from expect
  CPython gives the expected output here: this is a Brevi bug, not your code.
  Rerun with sandbox false to get CPython's answer, or keep going: Brevi will use CPython for this file.
```

and that same code (not every later program with the same file name) is marked CPython-only for the rest of the session. This needs CPython to run the case, which the
sandbox cannot hold, so it happens only when `sandbox` is false, or on Linux where Landlock holds CPython to the
same read and write grants (v0.26.0). Under the sandbox elsewhere, the reply says the case was not checked against
CPython.

### 3. MCP `run` without cases

Defaults to `verify: true` with the same rules as `--verify` above, so an agent that never heard of `verify` still
gets a checked answer. `verify: false` turns it off. With `sandbox: false` (only on a `brevi mcp --trust-calls`
server) the check is against CPython. Under the sandbox, which is the default, CPython can't be held, so the check
is against Brevi's dynamic build (see 3a).

### 3a. The sandboxed check, against the dynamic build

From v0.27.0 the MCP `run` tool under the sandbox, and `brevi run --verify --sandbox [--allow ...]` on the CLI (which
used to refuse `--sandbox`), compare the typed build's answer with Brevi's dynamic build (`BREVI_DYN=1`) instead of
CPython. Both are built under the same grants and run under the same limits.

- **Same** stdout and exit code: the last line is `{"verify":"identical","against":"dynamic",...}`.
- **Different:** the answer given is the dynamic build's, with a line
  `{"verify":"differs","against":"dynamic","on":"stdout","line":N,"typed":"...","dynamic":"..."}`.
- **Not compared:** a program that does not take the typed build, reads the clock, unseeded random or the environment,
  writes files or uses the network, or that the dynamic build cannot build or crashes on, gets
  `{"verify":"refused","against":"dynamic","why":"..."}` and the program's own answer.

### 4. The certificate

The certificate `verify.rs` prints today, plus the run that produced it:

```json
{"verify":"identical","file":"main.py","sha256":"...","inputs":["sha256:..."],"args":[],"path":"typed",
 "exit":0,"stdout_same":true,"stderr_same":true,"brevi_ms":3,"cpython_ms":41,"speedup":13.7,
 "brevi":"0.27.0","cpython":"3.13.13"}
```

It goes to stderr on the CLI, and as the last line of the MCP reply. It is small enough for an agent to keep in context
and enough for a harness to log.

### 5. `brevi report`

Every `differs` is written to `$BREVI_CACHE/differs/` with the program, input and both outputs (local only).
`brevi report` prints them and a ready-to-file GitHub issue body. Nothing is sent anywhere.

## What it is not

- Not a replacement for conformance: it checks one program on the inputs it was given.
- Not a sandbox feature: it runs CPython, which the sandbox cannot hold outside Linux. Under the sandbox the check is against the dynamic build, which is a weaker check than CPython (both builds are Brevi's).
- Not free: the cost is one CPython run per verified run. On the release-test tasks CPython runs in 30 to 60 ms,
  which an agent does not notice; on heavy programs `--verify` should be used on the test inputs, not on every run.

## Acceptance

- `brevi run --verify` and the MCP default ship, with tests in `tests/check.sh` (a fake CPython that differs, as
  `tests/verify/fakepy.sh` does today) and `tests/mcp_test.py`.
- The skill, `llms.txt` and the agent docs say to use `brevi run --verify` (or the MCP default) as the recommended
  agent invocation.
- The release test logs how many checks `verify` caught (`differs`) per run; any non-zero number is a P0 bug.

## Open questions

1. Should `--verify` become the default for `brevi run` itself (opt out with `--no-verify`) once the daemon makes the
   native side free? Proposed: decide at the Phase 1 gate, on the measured cost.
2. Should a `differs` fall back to CPython silently in the MCP reply, or stop and ask? Proposed: fall back and say so in
   one line, since stopping costs the agent a turn.

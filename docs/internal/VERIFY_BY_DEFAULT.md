# Verify-by-default: design

**Status:** proposal for v0.27+ · **Owner:** maintainer (semantics) + second human (implementation) · **Why:** this is the trust contract the roadmap is built on. It converts "we are exact" (unprovable, and currently false in 44 known cases) into "we checked this program against CPython on your inputs" (provable per program).

## 1. Contract

> For each input, the native build and CPython must produce **byte-identical stdout** and the **same exit status**. If they differ, the program is **not certified**, and the run reports the first difference.

That is the whole promise. It is honest, checkable, and useful to an agent: a certificate is a fact about the program it just wrote.

## 2. Surfaces

| Surface | Behaviour |
|---|---|
| `brevi verify x.py --input DIR` | Existing. Keep as the explicit tool; align its certificate format with this doc. |
| `brevi run x.py --verify` | New. Runs each input twice (native, CPython) before reporting; prints the certificate as one JSON line on stderr; stdout is the native output when certified. |
| `brevi run x.py --verify --verify-input DIR` | Inputs are files (stdin text) in `DIR`, run in sorted order. Without it, `--verify` uses stdin once. |
| `brevi run --verify-strict` | On mismatch: exit 1, print the diff, do not pretend the run succeeded. |
| MCP `run` with `cases` | Verifies each case by default from v0.28. The result includes `verified` and the certificate in `structuredContent`. |
| MCP `verify` | Unchanged, for explicit certification without running the program as the agent's result. |

## 3. Semantics (decide these once)

| Behaviour | Decision |
|---|---|
| stdout | Byte-exact comparison. |
| exit status | Exact integer comparison. |
| stderr | **Not compared by default**; `--verify-stderr` compares it with traceback frames normalised away. This matches the documented "stderr is not part of the same-output promise". |
| nondeterminism | Run CPython **twice** with different `PYTHONHASHSEED` values; if its outputs differ, mark the case `unstable` and do not certify or fail. Run native once. (Optional `--verify-stable` to run native twice too.) |
| timeouts | Per-case timeout (default 10 s, `--verify-timeout`). A timeout is `timeout`, not certified. |
| limits | `--limit` applies to both sides; a limit hit is `limit`, not a mismatch. |
| side effects | V1: inputs and outputs are stdin/stdout only. Programs that write files or use the network are **not certifiable** — document it, and recommend `brevi verify --input` on programs whose tests are stdin/stdout. V2: `--verify-files` runs both sides in separate temp dirs and hashes the trees. |
| sandbox | Verification runs CPython, so it cannot run inside the program sandbox. In the MCP server, verification requires `sandbox: false` (as `verify` does today). Document that `run --verify --sandbox` is refused; the caller must choose. |
| multiple files | `--verify` follows imports as `brevi run` does; the certificate hashes every source file. |

## 4. Certificate

One JSON object on stderr (or in `structuredContent` for MCP):

```json
{
  "verified": true,
  "brevi": "0.27.0",
  "python": "3.13.13",
  "path": "typed",
  "files": {"main.py": "sha256:…", "work.py": "sha256:…"},
  "inputs": [{"name": "case-1", "sha256": "…"}],
  "cases": [
    {
      "input": "case-1",
      "stdout_sha256": "…",
      "exit_native": 0,
      "exit_cpython": 0,
      "match": true,
      "ms_native": 3.4,
      "ms_cpython": 22.1,
      "status": "matched"
    }
  ],
  "unstable": false,
  "stderr_compared": false
}
```

`status` is one of `matched`, `wrong_output`, `wrong_exit`, `unstable`, `timeout`, `limit`, `not_built`.

On mismatch, the stderr line also carries:

```json
{"verified": false, "case": "case-1", "first_diff": {"stream": "stdout", "line": 3, "expected": "42", "got": "43"}}
```

Never print the full outputs in the certificate; the caller already has them.

## 5. On-mismatch behaviour

- **Default (`brevi run --verify`):** run CPython to completion, return its stdout/exit status (so the caller gets the correct result), print the certificate with `verified:false` on stderr. This is the safe default for agents.
- **`--verify-strict`:** exit 1 with the diff and no stdout. Use in CI.
- **MCP:** return `verified:false` and the native result plus the diff; the harness decides. Do not silently substitute CPython unless `fallback: true` is passed.

## 6. Cost

Verification is 2–3 executions per input (native + two CPython runs to detect instability). Mitigate:

- `--verify-timeout` and `--verify-case-limit N` (first N inputs only).
- Skip the second CPython run with `--verify-fast` (loses unstable detection).
- In MCP, verify only cases marked `expect`-bearing; the server already builds once.
- Publish the overhead in the docs: "verify roughly doubles runtime; use it when the program's correctness matters more than its speed."

## 7. Rollout

| Version | Change |
|---|---|
| v0.27 | `brevi run --verify` and `--verify-input`; align `brevi verify` certificate; docs; acceptance tests. |
| v0.28 | MCP `run` with `cases` verifies by default; `fallback` flag; certificate in `structuredContent`. |
| v0.29 | Skill/editor rules recommend `--verify` in the agent loop; default for `brevi run` only when `--verify-input` is NIS (`BREVI_VERIFY=1` opt-in). |

Do not make it default for bare `brevi run` without inputs: there is nothing to verify against, and the double cost would be paid for nothing.

## 8. Acceptance tests

- [ ] Native and CPython agree on stdout+exit → `verified:true`, certificate hashes stable across runs.
- [ ] Program that prints a set of strings → `unstable`, not a failure.
- [ ] Program that raises → exit codes compared, `wrong_exit` when different.
- [ ] Program that writes a file → refused as not certifiable (V1) with a clear message.
- [ ] Timeout → `timeout`, not `wrong_output`.
- [ ] `--verify-strict` mismatch → exit 1, diff names the line.
- [ ] MCP `run` with cases → certificate present, `sandbox:false` required and documented.
- [ ] Verification overhead measured and published (target: <3x for stdin/stdout programs).

## 9. Open questions

1. Do we certify only when the program has tests/inputs, or also on a single stdin? Recommendation: only with inputs; one stdin is a weak sample and invites overclaiming.
2. Is `stderr` ever part of the contract? Recommendation: no; keep the documented scope, but `--verify-stderr` for users who want it.
3. How do file-writing programs get certified? V2 temp-dir hashing; do not promise it before it exists.
4. Does the certificate belong in the program's output directory? No — stderr/MCP only; keep stdout clean.

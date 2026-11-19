# Roadmap
<!-- description: Where Brevi is going: one fast binary from a Python MCP server or CLI. What comes next, what Brevi has stopped doing, and what is parked. -->

Brevi's current focus is one thing: turn a small Python MCP server or CLI into one binary that needs no Python on the machine that runs it, and starts fast. The guide is [Ship an MCP server or CLI as one binary](/docs/bundle-mcp), and the measured results are on [Showcases](/docs/showcases). This page lists what is planned, not promised dates.

## What Brevi has stopped doing

Brevi has stopped chasing full CPython parity and broad library clones. Programs it can't build natively are refused with a reason, or run through the CPython bridge, and new native ports of libraries are not planned. The hosted service and the paid plan are parked. The extra features that remain (`brevi run --verify`, `brevi mcp` tools, the sandbox, the SDK) are kept working, and their reference docs stay.

## Next

In this order:

1. **A real-server test.** Build popular MCP servers and CLIs unchanged, check they answer as under CPython, and publish the raw results on [Showcases](/docs/showcases), failures included.
2. **A memory-safety fix in the typed build.**
3. **Binary size.** First a baseline, then smaller release binaries.
4. **Memory that stays flat in long-running servers.** A built program does not free some memory yet (reference cycles, some typed objects), which is fine for short sessions but matters for a server that runs for days.
5. **Clearer build output.** Which path the build took, whether the binary needs Python, and the `claude mcp add` line to register it.
6. **macOS test coverage.** Brevi is released for macOS, but CI does not yet test the programs it builds there.
7. **Static Linux binaries.** Under consideration. Linux binaries link the system glibc dynamically today, so build on the oldest Linux you target.

## Security

We have not had an external audit; we are seeking funding for one. Until then, do not run hostile code under the sandbox: it is a policy layer, not an isolation boundary, so run untrusted code in a container or VM. Pilots with design partners running their own trusted code in their own container do not wait on it; a public beta should have a fixed-scope independent review first; the sandbox will not be offered as a boundary for hostile code before a paid audit. [Where Brevi stands](https://github.com/ets-public/brevi/blob/main/SECURITY.md#where-brevi-stands).

## Earlier work

The older plan aimed at conformance targets, edit-loop latency and agent harness adoption. Those measures are still tracked in [docs/ROADMAP.md](https://github.com/ets-public/brevi/blob/main/docs/ROADMAP.md) and [docs/METRICS.md](https://github.com/ets-public/brevi/blob/main/docs/METRICS.md), but they are no longer the focus. Figures for how closely Brevi matches CPython are on [What works, and limits](/docs/compatibility#in-numbers).

### The earlier plan's numbers

Kept for the record; they are no longer what Brevi steers by.

Each measure links to where its number comes from. Notes on the marked numbers are under the tables.

#### Correct answers

| Measure | Now | In 90 days | In 12 months |
|---|---|---|---|
| [CPython conformance](https://github.com/ets-public/brevi/blob/main/bench/conformance/report.md): tests whose output matches CPython's | **97.3%** of the scored suite, with 83 string-as-code tests excluded (1043/1072)<sup>5</sup> | 70% | 90% |
| [Wrong answers](https://github.com/ets-public/brevi/tree/main/compat/conformance_known_wrong): built, but answered differently | **0 tests**<sup>5</sup> | ≤ 25 | 0 |

#### Waiting on each edit (median)

| Measure | Now | In 90 days | In 12 months |
|---|---|---|---|
| [`brevi check --fast`](https://github.com/ets-public/brevi/blob/main/docs/LATENCY.md) | **22 ms**<sup>1</sup> | 20 ms | 10 ms |
| [`brevi check`](https://github.com/ets-public/brevi/blob/main/docs/LATENCY.md) | **94 ms**<sup>1</sup> | 50 ms | 30 ms |
| [Build](https://github.com/ets-public/brevi/blob/main/docs/LATENCY.md) | **0.91 s**<sup>1</sup> | 0.5 s | 0.3 s |
| [`brevi run`](https://github.com/ets-public/brevi/blob/main/docs/LATENCY.md) (quick first build) | **0.35 s**<sup>1</sup> | | |

#### Agents

| Measure | Now | In 90 days | In 12 months |
|---|---|---|---|
| Saved agent programs on the fast typed path | **70%** (567/813)<sup>2</sup> | 80% | 90% |
| First check builds natively ([release test](https://github.com/ets-public/brevi/blob/main/docs/METRICS.md)) | **24 of 24**<sup>3</sup> | 80% | 90% |
| [Cost to a passing answer](https://github.com/ets-public/brevi/blob/main/docs/METRICS.md), relative to CPython | **1.04x** Haiku<br>**1.03x** Sonnet<sup>3</sup> | ≤ 1.10x | ≤ 1.00x |

#### Adoption

| Measure | Now | In 90 days | In 12 months |
|---|---|---|---|
| Install to first native run, no Rust | **Not timed**<sup>4</sup> | 60 s | 60 s |
| Harnesses or sandboxes that ship Brevi | **0** | 1 | 3 |

**Notes**

1. **Measured on main with fixes that shipped in v0.28.0.** In v0.27.0 and v0.27.1 a new safety check wrongly sent the test program to the slower dynamic build, so these were 160 ms, 160 ms, 1.18 s and 0.43 s. At v0.26.0 they were 16 ms, 94 ms and 0.87 s; the safety check still costs about 6 ms of `check --fast`.
2. Measured at v0.25.0.
3. Measured at v0.26.0, one run each with Haiku and Sonnet, with Brevi's errors shown to the model.
4. Possible since v0.27.0 with the [brevi-sandbox image](https://github.com/ets-public/brevi/blob/main/docs/SDK_AND_IMAGES.md).
5. At v0.28.2, 16 November (CPython's core tests, at `PYTHONHASHSEED=0`; the 83 that run a string as code are left out as refused by design). The standard-library slice is 96.9% (1989 of 2053) with none wrong. That covers the 21 files scored (ten added from 12 to 14 November: `test_set` and `test_patma` in core, `test_dataclasses` and `test_enum` in stdlib among them); more files are measured for planning but not scored yet. At v0.27.0: 80.4% (250 of 311) and 8 wrong; at v0.26.0, 44 wrong.

Unless marked, numbers are from v0.27.0 (11 November 2026). Edit times are from one 4-core Xeon @ 2.10 GHz, the same kind of machine as the v0.26.0 numbers.

Two goals of the earlier plan, also no longer the focus:

- **Conformance in CI.** The CPython conformance run becomes a release gate that fails on any new wrong result, with a public dashboard and the known-wrong list burned down in public, one fix at a time. Shipped in v0.27.0: `tools/conformance.py --check` fails CI on any new wrong result, and conformance is up from 58.2% to 80.4% (250 of 311). From v0.28.0: every known-wrong entry names an issue, an owner and an expiry date, and the release itself does not ship below core 90%, stdlib 80% or with any known-wrong result (`--release-gate` runs in release.yml). On main (14 November) core reads 97.4% and stdlib 92.1% with no wrong answers, after ten more of CPython's test files were scored; the next step is scoring more of them (`--candidates` measures the rest).
- **1.0** when conformance reaches 90% with no known wrong results, the typed path covers 90% of agent programs, `brevi check` answers in 30 ms, an outside security review is done, and three harnesses ship Brevi. Until then Brevi stays 0.x.

## Shipped recently

Each item is in the [changelog](/docs/changelog) with its release.

- **In v0.28.3:** security fixes: every parsed import checked under the sandbox, the child-process code checks the native grant and Brevi's internal modules cannot be imported (a `--sandbox` escape closed); unknown options refused; seccomp keeps sandboxed CPython from other processes on Linux; sandboxed CPython refused where a kernel lock is missing unless `--allow-unlocked-python`; no grant reaches Brevi's caches and no shared cache or cargo home is trusted; `brevi mcp --http` backs off wrong tokens; crate lines checked; release supply chain pinned.
- **In v0.28.2:** security fixes for temp folders (private to the user on Linux and other unix) and `brevi mcp --http` (requests bounded before the token check); on Windows, CRLF source, source paths without `\\?\`, the cp1252 codec and `re.LOCALE`, and the standard-library conformance slice gates a release.
- **In v0.28.1:** `re.IGNORECASE` and str case methods answer with CPython 3.13's Unicode tables on any Rust toolchain (Rust 1.100 pinned); `namedtuple` names checked as CPython checks them; run-time regex patterns the typed matcher lacks re-run on the dynamic build; Windows `--standalone`, the Windows certificate store, Windows ARM64, and Windows conformance as a release gate.
- **In v0.28.0:** `--strict`; core conformance 97.3% (of the scored suite, with 83 string-as-code tests excluded) and stdlib 96.9% over 21 files with no wrong answers; `match` statements; complex numbers, metaclasses, `def f[T]`, threads that take turns, `weakref.proxy`, strs with lone surrogates, and exact `vars()`/`dir()` in dynamic builds; releases held to the conformance gates, with an SBOM, provenance attestations and a reproducible-build check; native `bytearray`, `memoryview`, `collections.abc`, `ChainMap` and the `User*` classes, `functools.singledispatch`, `partialmethod` and `cached_property`, `string.Template`, `itertools.tee` and `batched`, warnings with the program's lines, and bytes patterns in `re`; sandboxed CPython gets no network without a grant; engine libraries pinned by hash; Windows paths as `ntpath`.
- **In v0.27.0:** `brevi run --verify`; the Python and Node SDKs, `brevi-python` and the published sandbox image; tiered builds after an edit; conformance 58.2% to 80.4% with a CI ratchet and a fuzzer; `eval()` of literal text, `__del__`, `weakref` and lazy itertools on the dynamic build; Windows library calls, generators, async, `--limit mem` and native requests, httpx, sqlite3, duckdb, onnxruntime and MCP servers; sandbox and MCP fixes from the 10 November review.
- **v0.25.0 and earlier:** the sandbox on every build path, caller-only grants, native numpy, the dynamic build for programs the typed build refuses, and `pip install brevi`, `npx brevi`, `cargo install brevi` and the MCP registry.

## Always


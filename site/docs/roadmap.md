# Roadmap
<!-- description: Where Brevi is going: the default runtime for AI-written Python. The numbers we steer by, what comes next, and what is paused until 1.0. -->

Brevi's goal is to be **the default runtime for the Python AI agents write**: the code an agent writes runs natively, checked against CPython, inside the sandboxes and SDKs agents already use. Models don't install compilers, so Brevi gets there when harness and sandbox teams know it, choose it, and ship it in their images by default. What stands in the way is trust in each program, the wait on every edit, install friction and distribution, not another 2x on a benchmark.

The full plan, with every decision and risk, is [docs/ROADMAP.md](https://github.com/ets-public/brevi/blob/main/docs/ROADMAP.md); each number below is defined in [docs/METRICS.md](https://github.com/ets-public/brevi/blob/main/docs/METRICS.md). This page lists what is planned, not promised dates.

## The numbers we steer by

Each measure links to where its number comes from. Notes on the marked numbers are under the tables.

### Correct answers

| Measure | Now | In 90 days | In 12 months |
|---|---|---|---|
| [CPython conformance](https://github.com/ets-public/brevi/blob/main/bench/conformance/report.md): tests whose output matches CPython's | **93.1%** (268/288)<sup>5</sup> | 70% | 90% |
| [Wrong answers](https://github.com/ets-public/brevi/tree/main/compat/conformance_known_wrong): built, but answered differently | **0 tests**<sup>5</sup> | ≤ 25 | 0 |

### Waiting on each edit (median)

| Measure | Now | In 90 days | In 12 months |
|---|---|---|---|
| [`brevi check --fast`](https://github.com/ets-public/brevi/blob/main/docs/LATENCY.md) | **22 ms**<sup>1</sup> | 20 ms | 10 ms |
| [`brevi check`](https://github.com/ets-public/brevi/blob/main/docs/LATENCY.md) | **94 ms**<sup>1</sup> | 50 ms | 30 ms |
| [Build](https://github.com/ets-public/brevi/blob/main/docs/LATENCY.md) | **0.91 s**<sup>1</sup> | 0.5 s | 0.3 s |
| [`brevi run`](https://github.com/ets-public/brevi/blob/main/docs/LATENCY.md) (quick first build) | **0.35 s**<sup>1</sup> | | |

### Agents

| Measure | Now | In 90 days | In 12 months |
|---|---|---|---|
| Saved agent programs on the fast typed path | **70%** (567/813)<sup>2</sup> | 80% | 90% |
| First check builds natively ([release test](https://github.com/ets-public/brevi/blob/main/docs/METRICS.md)) | **24 of 24**<sup>3</sup> | 80% | 90% |
| [Cost to a passing answer](https://github.com/ets-public/brevi/blob/main/docs/METRICS.md), relative to CPython | **1.04x** Haiku<br>**1.03x** Sonnet<sup>3</sup> | ≤ 1.10x | ≤ 1.00x |

### Adoption

| Measure | Now | In 90 days | In 12 months |
|---|---|---|---|
| Install to first native run, no Rust | **Not timed**<sup>4</sup> | 60 s | 60 s |
| Harnesses or sandboxes that ship Brevi | **0** | 1 | 3 |

**Notes**

1. **Measured on main with fixes that ship in the next release.** In v0.27.0 and v0.27.1 a new safety check wrongly sent the test program to the slower dynamic build, so these were 160 ms, 160 ms, 1.18 s and 0.43 s. At v0.26.0 they were 16 ms, 94 ms and 0.87 s; the safety check still costs about 6 ms of `check --fast`.
2. Measured at v0.25.0.
3. Measured at v0.26.0, one run each with Haiku and Sonnet, with Brevi's errors shown to the model.
4. Possible since v0.27.0 with the [brevi-sandbox image](https://github.com/ets-public/brevi/blob/main/docs/SDK_AND_IMAGES.md).
5. On main, from the next release (CPython's core tests; the 25 that run a string as code are left out as refused by design). At v0.27.0: 80.4% (250 of 311) and 8 wrong; at v0.26.0, 44 wrong. The standard-library slice is 83.9% on main, with 5 wrong.

Unless marked, numbers are from v0.27.0 (11 November 2026). Edit times are from one 4-core Xeon @ 2.10 GHz, the same kind of machine as the v0.26.0 numbers.

The north star is the share of real agent-written programs that build natively, match CPython on their own tests, and do so on the first check. It needs real agent traffic, so it starts with the alpha below.

## Now (the next 30 days)

- **Tell the truth everywhere.** One source for every public number, no absolute claims, measured speed ranges, and the conformance score first in the numbers table.
- **Conformance in CI.** The CPython conformance run becomes a release gate that fails on any new wrong result, with a public dashboard and the known-wrong list burned down in public, one fix at a time. Shipped in v0.27.0: `tools/conformance.py --check` fails CI on any new wrong result, and conformance is up from 58.2% to 80.4% (250 of 311). From the next release: every known-wrong entry names an issue, an owner and an expiry date, and the release itself does not ship below core 90%, stdlib 80% or with any known-wrong result (`--release-gate` runs in release.yml). On main core already reads 93.1% with no wrong answers, and stdlib 83.9% with 5.
- **Fuzzing.** Generated programs run natively and under CPython and compared; a crash is a top-priority bug, and a signal is treated as a security bug until it is understood. Shipped in v0.27.0: `tools/fuzz.py` writes seeded random agent-style Python, including shared-list cases, and compares the builds with CPython.
- **Verify by default.** `brevi verify` (v0.26.0) checks a program against CPython on your inputs and prints a certificate. Next, `brevi run --verify` and the MCP `run` tool check every run where it is safe and cheap, and answer with CPython's output if the two ever differ. [Design](https://github.com/ets-public/brevi/blob/main/docs/VERIFY_BY_DEFAULT.md). Shipped in v0.27.0: `brevi run --verify` and the MCP `run` tool do this.
- **Measure the loop.** `bench/latency.py` times every command an agent runs after an edit, and the release test now records how often the first check builds natively and what it costs to reach a passing answer with Brevi's errors shown to the model.
- **A private alpha.** Ten harness authors and agent-platform teams running Brevi weekly, with opt-in metrics only. Brevi sends no telemetry by default.

## Next (to the private alpha gate)

- **Faster builds after an edit.** Build at low optimisation for the edit loop and optimise in the background, and keep `rustc`'s incremental state for `brevi check`; a warm daemon after that. [Spike results](https://github.com/ets-public/brevi/blob/main/docs/LATENCY.md). Shipped in v0.27.0: runs after an edit build at opt-level 0 with opt-level 1 in the background, and library copies are compiled once and shared.
- **An SDK.** `brevi.run(code, stdin=..., limits=...)` for Python, then Node, over the MCP server, so a harness never shells out. Sandboxed by default; only the caller grants anything. [Design](https://github.com/ets-public/brevi/blob/main/docs/SDK_AND_IMAGES.md). Shipped in v0.27.0: `import brevi` (Python) and `import { run } from "brevi"` (Node).
- **A `python3` drop-in.** `brevi-python` behaves like `python3`, builds natively when it can and is CPython otherwise. Shipped in v0.27.0; on macOS it works from v0.27.1.
- **Published base images.** One image with Brevi preinstalled for [E2B, Daytona, Modal, Cloudflare and Docker](/docs/agents#agent-platforms-e2b-daytona-modal-cloudflare), published with each release, so the [templates](/docs/agents#agent-platforms-e2b-daytona-modal-cloudflare) need no build. Shipped in v0.27.0: `ghcr.io/ets-public/brevi-sandbox`.
- **More programs on the fast path.** A published list of why programs leave the typed build, worked down from the top, then typed builds per function or module so one changing variable does not slow the whole file.
- **Other model families.** The release test on at least one OpenAI, one Google and one open-weight model, not Claude alone. DeepSeek runs in the release test from v0.27.0 (report only).
- **A real-task benchmark.** A public agent-task suite run with and without Brevi: success, time, tokens and dollars per task, with the raw logs and a script to reproduce it.

## Later (to 1.0)

- **brevi.run, hosted.** A closed beta of the compiler as a sandboxed API for agents, only once the platform's VM or microVM isolation and an outside security review are in place.
- **Windows parity.** From v0.27.0, library programs, generators and `async` build on Windows (CPython bridge and Win32 fibers). From the next release, `os.path` is `ntpath` and `sys.platform` is `win32` on Windows, and conformance runs on Windows every week. `--standalone` on Windows and macOS is next.
- **1.0** when conformance reaches 90% with no known wrong results, the typed path covers 90% of agent programs, `brevi check` answers in 30 ms, an outside security review is done, and three harnesses ship Brevi. Until then Brevi stays 0.x.

## Paused until 1.0

- **New native library ports.** numpy, polars, duckdb, sqlite3, onnxruntime, safetensors, jinja2, pydantic, httpx, the LLM SDKs and the tokenizers stay maintained; other libraries run in CPython through the bridge.
- **New `.bv` language features.** Brevi's own language is in maintenance; the goal is plain Python.
- **GUI and tkinter work** beyond bug fixes.

## Shipped recently

Each item is in the [changelog](/docs/changelog) with its release.

- **On main, from the next release:** `--strict`; core conformance 93.1% with no wrong answers and stdlib 83.9%; releases held to the conformance gates, with an SBOM, provenance attestations and a reproducible-build check; native `bytearray`, `memoryview`, `collections.abc`, `ChainMap` and the `User*` classes, `functools.singledispatch`, `partialmethod` and `cached_property`, `string.Template`, `itertools.tee` and `batched`, warnings with the program's lines, and bytes patterns in `re`; sandboxed CPython gets no network without a grant; engine libraries pinned by hash; Windows paths as `ntpath`.
- **In v0.27.1:** `brevi-python` works on macOS; releases run the whole suite on the commit they release, check every archive they ship and check Windows on the shipped zip.
- **In v0.27.0:** `brevi run --verify`; the Python and Node SDKs, `brevi-python` and the published sandbox image; tiered builds after an edit; conformance 58.2% to 80.4% with a CI ratchet and a fuzzer; `eval()` of literal text, `__del__`, `weakref` and lazy itertools on the dynamic build; Windows library calls, generators, async, `--limit mem` and native requests, httpx, sqlite3, duckdb, onnxruntime and MCP servers; sandbox and MCP fixes from the 10 November review.
- **v0.26.0:** `brevi verify`; resource limits (`--limit`); folder and host grants (`--allow-read`, `--allow-write`, `--allow-net`); Landlock holding CPython under the sandbox on Linux; agent platform templates and the [weekly benchmark](/docs/benchmarks#weekly-agent-code-benchmark); native async; native tiktoken, pydantic, jinja2, polars, sqlite3, duckdb, safetensors, onnxruntime, httpx and the Anthropic and OpenAI SDKs; `.bv` files using the same native libraries; MCP over HTTP.
- **v0.25.0 and earlier:** the sandbox on every build path, caller-only grants, native numpy, the dynamic build for programs the typed build refuses, and `pip install brevi`, `npx brevi`, `cargo install brevi` and the MCP registry.

## Always

- Every saved program that once passed keeps passing: the full set is rerun on every change and on every new Rust and Python release.
- A silent difference from CPython is treated as the worst kind of bug. The known ones are listed in public, and the list only gets shorter.
- Every public number links to where it comes from, with its date and hardware.
- One release a week, on Mondays, after that week's release test passes. Changes land on `main` under "Unreleased" in the [changelog](/docs/changelog) until then.

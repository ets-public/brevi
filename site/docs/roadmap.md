# Roadmap
<!-- description: Where Brevi is going: the default runtime for AI-written Python. The numbers we steer by, what comes next, and what is paused until 1.0. -->

Brevi's goal is to be **the default runtime for the Python AI agents write**: the code an agent writes runs natively, checked against CPython, inside the sandboxes and SDKs agents already use. Models don't install compilers, so Brevi gets there when harness and sandbox teams know it, choose it, and ship it in their images by default. What stands in the way is trust in each program, the wait on every edit, install friction and distribution, not another 2x on a benchmark.

The full plan, with every decision and risk, is [docs/ROADMAP.md](https://github.com/ets-public/brevi/blob/main/docs/ROADMAP.md); each number below is defined in [docs/METRICS.md](https://github.com/ets-public/brevi/blob/main/docs/METRICS.md). This page lists what is planned, not promised dates.

## The numbers we steer by

Measured on v0.26.0, 9 November 2026. Latency is from one machine (4-core Xeon @ 2.10 GHz); every number links to where it comes from.

| | Now | In 90 days | In 12 months |
|---|---:|---:|---:|
| [CPython conformance](https://github.com/ets-public/brevi/blob/main/bench/conformance/report.md) (tests that print exactly what CPython prints from a native build) | 58.2% (181 of 311) | 70% | 90% |
| [Known wrong results](https://github.com/ets-public/brevi/blob/main/compat/conformance_known_wrong.txt) (built, but answered differently from CPython) | 44 tests | 25 or fewer | 0 |
| [`brevi check --fast`](https://github.com/ets-public/brevi/blob/main/docs/LATENCY.md) after an edit, median | 16 ms | 20 ms or less | 10 ms |
| [`brevi check`](https://github.com/ets-public/brevi/blob/main/docs/LATENCY.md) after an edit, median | 94 ms | 50 ms | 30 ms |
| [Build after an edit](https://github.com/ets-public/brevi/blob/main/docs/LATENCY.md), median | 0.87 s typed, 1.5 s dynamic | 0.5 s | 0.3 s |
| Saved agent programs on the fast typed path | 567 of 813 (70%, v0.25.0) | 80% | 90% |
| First check that builds natively, on the release test | measured from 9 November | 80% | 90% |
| Cost to a passing answer vs CPython, with Brevi's errors shown to the model | measured from 9 November | 1.10x or less | 1.00x or less |
| Install to a first native run with no Rust installed | not possible yet | 60 s | 60 s |
| Harnesses or sandbox platforms that ship Brevi | 0 | 1 | 3 |

The north star is the share of real agent-written programs that build natively, match CPython on their own tests, and do so on the first check. It needs real agent traffic, so it starts with the alpha below.

## Now (the next 30 days)

- **Tell the truth everywhere.** One source for every public number, no absolute claims, measured speed ranges, and the conformance score first in the numbers table.
- **Conformance in CI.** The CPython conformance run becomes a release gate that fails on any new wrong result, with a public dashboard and the known-wrong list burned down in public, one fix at a time.
- **Fuzzing.** Generated programs run natively and under CPython and compared; a crash is a top-priority bug, and a signal is treated as a security bug until it is understood.
- **Verify by default.** `brevi verify` (v0.26.0) checks a program against CPython on your inputs and prints a certificate. Next, `brevi run --verify` and the MCP `run` tool check every run where it is safe and cheap, and answer with CPython's output if the two ever differ. [Design](https://github.com/ets-public/brevi/blob/main/docs/VERIFY_BY_DEFAULT.md).
- **Measure the loop.** `bench/latency.py` times every command an agent runs after an edit, and the release test now records how often the first check builds natively and what it costs to reach a passing answer with Brevi's errors shown to the model.
- **A private alpha.** Ten harness authors and agent-platform teams running Brevi weekly, with opt-in metrics only. Brevi sends no telemetry by default.

## Next (to the private alpha gate)

- **Faster builds after an edit.** Build at low optimisation for the edit loop and optimise in the background, and keep `rustc`'s incremental state for `brevi check`; a warm daemon after that. [Spike results](https://github.com/ets-public/brevi/blob/main/docs/LATENCY.md).
- **An SDK.** `brevi.run(code, stdin=..., limits=...)` for Python, then Node, over the MCP server, so a harness never shells out. Sandboxed by default; only the caller grants anything. [Design](https://github.com/ets-public/brevi/blob/main/docs/SDK_AND_IMAGES.md).
- **A `python3` drop-in.** `brevi-python` behaves like `python3`, builds natively when it can and is CPython otherwise.
- **Published base images.** One image with Brevi preinstalled for [E2B, Daytona, Modal, Cloudflare and Docker](/docs/agents#agent-platforms-e2b-daytona-modal-cloudflare), published with each release, so the [templates](/docs/agents#agent-platforms-e2b-daytona-modal-cloudflare) need no build.
- **More programs on the fast path.** A published list of why programs leave the typed build, worked down from the top, then typed builds per function or module so one changing variable does not slow the whole file.
- **Other model families.** The release test on at least one OpenAI, one Google and one open-weight model, not Claude alone.
- **A real-task benchmark.** A public agent-task suite run with and without Brevi: success, time, tokens and dollars per task, with the raw logs and a script to reproduce it.

## Later (to 1.0)

- **brevi.run, hosted.** A closed beta of the compiler as a sandboxed API for agents, only once the platform's VM or microVM isolation and an outside security review are in place.
- **Windows parity** for library programs, and `--standalone` on macOS.
- **1.0** when conformance reaches 90% with no known wrong results, the typed path covers 90% of agent programs, `brevi check` answers in 30 ms, an outside security review is done, and three harnesses ship Brevi. Until then Brevi stays 0.x.

## Paused until 1.0

- **New native library ports.** numpy, polars, duckdb, sqlite3, onnxruntime, safetensors, jinja2, pydantic, httpx, the LLM SDKs and the tokenizers stay maintained; other libraries run in CPython through the bridge.
- **New `.bv` language features.** Brevi's own language is in maintenance; the goal is plain Python.
- **GUI and tkinter work** beyond bug fixes.

## Shipped recently

Each item is in the [changelog](/docs/changelog) with its release.

- **v0.26.0:** `brevi verify`; resource limits (`--limit`); folder and host grants (`--allow-read`, `--allow-write`, `--allow-net`); Landlock holding CPython under the sandbox on Linux; agent platform templates and the [weekly benchmark](/docs/benchmarks#weekly-agent-code-benchmark); native async; native tiktoken, pydantic, jinja2, polars, sqlite3, duckdb, safetensors, onnxruntime, httpx and the Anthropic and OpenAI SDKs; `.bv` files using the same native libraries; MCP over HTTP.
- **v0.25.0 and earlier:** the sandbox on every build path, caller-only grants, native numpy, the dynamic build for programs the typed build refuses, and `pip install brevi`, `npx brevi`, `cargo install brevi` and the MCP registry.

## Always

- Every saved program that once passed keeps passing: the full set is rerun on every change and on every new Rust and Python release.
- A silent difference from CPython is treated as the worst kind of bug. The known ones are listed in public, and the list only gets shorter.
- Every public number links to where it comes from, with its date and hardware.
- One release a week, on Mondays, after that week's release test passes. Changes land on `main` under "Unreleased" in the [changelog](/docs/changelog) until then.

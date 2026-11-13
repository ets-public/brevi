# Roadmap: the default runtime for AI-written Python

**Adopted:** 9 November 2026 · **Baseline:** v0.26.0 · **Team:** two humans and Claude Code, no funding yet.
**Metrics:** every number here is defined in [METRICS.md](METRICS.md), with how it is measured and where the
current value comes from. The public summary is [brevi-lang.org/docs/roadmap](https://brevi-lang.org/docs/roadmap)
(source `site/docs/roadmap.md`).

This roadmap came out of an outside adversarial review of v0.26.0. Its baselines were rechecked against the repo
on 9 November 2026; where the review's figure and ours differ, ours is given with its source.

> **The premise.** Brevi does not become "the default for AI models" because a model prefers it: models don't
> install compilers. It gets there in three steps: harness and sandbox vendors **know** it, **choose** it, and
> **ship it by default** in their images and SDKs. The compiler is good enough to start. What holds it back is
> per-program trust, loop latency, install friction and distribution, not another 2x on a microbenchmark.

## If we only do three things

1. **Verify by default.** Every agent run can check the native build against CPython on the program's own inputs
   and report a certificate or the first difference. No other Python compiler offers that, and it turns today's
   58.2% conformance score from a weakness into a product feature. (`brevi verify` exists since v0.26.0; the work
   is making it the default.)
2. **A fast inner loop with no `rustc` in it.** `brevi check --fast` already answers in about 16 ms; the full check
   (94 ms) and a build after an edit (0.87 s typed, 1.5 s dynamic) are what an agent waits for. A warm daemon, and
   later a dev backend, take `rustc` out of the edit loop; full codegen stays for `brevi build`.
3. **Be in the image, not on the website.** A harness SDK, a `python3` shim, and Brevi preinstalled in the E2B,
   Daytona, Modal and Cloudflare base images. That is the only realistic way to adoption nobody has to opt into.

## Decisions (9 November 2026)

| Question | Decision |
|---|---|
| Hosted service or SDK first? | **SDK and base images first.** A hosted beta (brevi.run) comes later, and only behind the WS7 isolation work. |
| Keep developing `.bv`? | **Frozen.** Maintenance and bug fixes only; no new language features, no constrained-decoding grammar. |
| Keep porting libraries? | **No new ports.** The existing native libraries (numpy, polars, duckdb, sqlite3, onnxruntime, safetensors, jinja2, pydantic, httpx, the LLM SDKs, tokenizers) are maintained; everything else is bridged to CPython. Revisit only if an alpha user shows a library is a must-have. |
| Move to 1.0? | **Stay 0.x** until the 1.0 gates below are met. The version number is used honestly. |
| Funding and the external security audit | Adam's call. Nothing in this plan waits on them until the hosted beta (Phase 2). |
| Which harness first? | Open: the one of Claude Code, Cursor, Codex CLI or Gemini CLI with the most permissive extension surface. One deep integration beats five shallow ones. |

## The four levels of "default"

| Level | What it means | Metric | Target |
|---|---|---|---|
| **Known** | Harness authors and agent-platform teams know Brevi exists | Qualified conversations; integration docs read | 10 teams by month 3 |
| **Chosen** | A harness or sandbox ships it as an option | Integrations merged in third-party repos | 3 by month 6 |
| **Default** | It is in the base image or toolchain with no user action | Weekly native builds from integrated platforms | 10k/week by month 12 |
| **Entrenched** | Removing it would be a regression for users | Median share of agent Python run natively | >50% by month 18 |

## North star and metric tree

**North star: Agent-Native Exactness Rate (ANER).** The share of real agent-written programs that build natively,
match CPython on their own tests, and do so on the first check. Fast, correct and cheap to iterate, in one number.

| Metric | v0.26.0 baseline | 90 days | 12 months | Measured by |
|---|---:|---:|---:|---|
| ANER (real agent code) | not measured | 60% | 90% | opt-in alpha traffic sample |
| Conformance (CPython 3.13.13 slice) | 99.3% (300/302; stdlib 654/655) on main, from the next release; 80.4% (250/311) at v0.27.0 | ≥70% | ≥90% | `tools/conformance.py` |
| Known-wrong allowlist | 0 on main; 8 tests, 16 test-mode pairs at v0.27.0 (44 and 88 at v0.26.0) | ≤25 tests | 0 | `compat/conformance_known_wrong/` |
| Crash or timeout (conformance) | 0 on main; 0 of 311 tests at v0.27.0 | 0 on fuzz corpus | 0 on fuzz corpus | fuzz harness (WS1) |
| `brevi check --fast` p50 / p95 | 22 / 24 ms on main (d91ca4b; ships in the next release); 160 / 174 ms at v0.27.0, when a false refusal sent the fixture dynamic; 16 / 18 ms at v0.26.0 | ≤20 ms p50 | ≤10 ms p50 | `bench/latency.py` |
| `brevi check` p50 / p95 | 94 / 107 ms on main; 160 / 209 ms at v0.27.0; 94 / 114 ms at v0.26.0 | ≤50 / ≤150 ms | ≤30 / ≤80 ms | `bench/latency.py` |
| Build after an edit, p50 | 0.91 s typed, 1.6 s dynamic on main; `brevi run` after an edit 0.35 s with tiered builds; 1.18 s at v0.27.0; 0.87 s typed, 1.5 s dynamic at v0.26.0 | ≤500 ms | ≤300 ms | `bench/latency.py` |
| Typed-path rate (agent corpus) | 567/813 (70%, v0.25.0) | 80% | 90% | `bench/typed_corpus.py` |
| First-attempt native build | 24/24 (Haiku and Sonnet, one run, errors shown) | 80% | 90% | `bench/e2e.py` (`n1`), alpha traffic |
| Tokens-to-green vs CPython, errors shown | 1.04x Haiku, 1.03x Sonnet (one run) | ≤1.10x | ≤1.00x | `bench/e2e.py --show-errors` |
| Install to first run, no Rust | possible with the `ghcr.io/ets-public/brevi-sandbox` image from v0.27.0; not yet timed | ≤60 s (container or hosted) | same | timed install test |
| Harness integrations | 0 | 1 | 3 | third-party repos |
| Weekly native builds (adoption) | about 0 | 100 (alpha) | 10k | opt-in counts |

Latency figures are from one machine (4-core Xeon @ 2.10 GHz, rustc 1.97.0), recorded in
`bench/latency-results/`. The review quoted 144 ms for `brevi check` (the README's figure for a new single file) and
1.2 s for a cold build; `bench/latency.py` replaces both with a fixed fixture and p50/p95.

**Rule:** every public number links to its artifact, a date and the hardware. No number without a source, and no
absolute correctness claim.

## Workstreams

Allocation of effort: 45% trust (WS1), 20% loop latency (WS2), 15% distribution (WS5), 10% fast path (WS3),
5% token economics (WS4), 5% evidence (WS6). WS7 is scheduled, not continuous.

### WS1: Trust, with conformance as a product surface (highest priority)

A silent wrong answer is a career risk for the harness that shipped it. The conformance number is a liability while
it is hidden and an asset once it is public, trending, and paired with per-run verification.

- Conformance in CI: nightly, plus a release gate that fails on any new wrong result.
- A public dashboard: conformance, known-wrong count, per-file breakdown, trend since v0.24.
- Fuzz the runtime for panics (program generation plus `tools/parity_probe.py`). A panic is a P0 bug; a signal is a
  security bug until triaged.
- Burn the allowlist down in public, one PR per entry where feasible.
- **Verify by default:** `brevi run --verify`, and the MCP `run` tool when tests are supplied, run native and CPython
  on the same inputs; on a difference, report the first differing line and fall back to CPython unless `--strict`.
  Design: [VERIFY_BY_DEFAULT.md](VERIFY_BY_DEFAULT.md).
- Widen the conformance slice beyond `test_grammar`, `test_scope`, `test_int`, `test_list` and `test_dict` to the
  stdlib modules agents use most: `json`, `re`, `datetime`, `collections`, `itertools`, `argparse`.

*Done when:* CI fails a PR that adds a wrong result not on the (shrinking) allowlist; the dashboard updates weekly
and is linked from the site; `--verify` is documented as the recommended agent invocation and is the MCP default when
tests are supplied; 30 days of fuzzing at about 10k programs a day with no panics.

### WS2: Loop latency

Agents edit and rerun tens of times per task. Runtime speed matters when tests are heavy; compile latency matters
every time.

- `brevi check --fast` (front end only, no `rustc`) is the default agent check unless a build is needed.
- A compiler daemon: a warm process, `rustc` incremental for the emit path, and the existing content-addressed cache.
- A dev backend: a timeboxed 3-week spike on Cranelift or an interpreter over the typed IR for `run` and `check`,
  behind a flag until it beats the daemon on p50. Full `rustc` only for `brevi build --release`.
- Ship the prebuilt runtime rlib in release archives so a first build is not 5 to 15 s.

*Done when:* `check --fast` p50 ≤20 ms and full check ≤50 ms on the 3-file fixture; an unchanged `brevi run` ≤10 ms;
build after an edit ≤500 ms; `bench/latency.py` in CI with a p50/p95 non-regression gate.

### WS3: Widen the fast path where agents write

The typed path is the speed, and whole-program demotion is the enemy: 70% of our agent corpus builds typed, but only
25 of the 230 conformance tests that built took the typed path, and the review measured the dynamic path at roughly
1.3x CPython. On the 12 compute benchmarks (12 Nov 2026) the dynamic build runs at 0.5x to 1.4x CPython, median 0.85x.

- `brevi check --explain` over real agent code (alpha users, with permission) and a published refusal-reason
  histogram; work down the top 20.
- The agent idiom list: list-of-dicts, `Optional`, closures and `nonlocal`, dataclasses, JSON-shaped values,
  class-heavy code, string building, generators.
- Hybrid typing: per-function or per-module specialization, so one type-changing variable does not demote the file.
- Dynamic-path speed and memory (shared with the object-memory work).

*Done when:* typed rate ≥80% on the 813 corpus and on alpha traffic within 90 days; every top-20 refusal reason has an
owner and a test.

### WS4: Token economics, measured with errors shown

The default release-test arm runs a program with CPython when Brevi refuses it and never shows the model why, so it
cannot measure the cost of actually using Brevi.

- The errors-shown arm (`bench/e2e.py --show-errors`, added 9 November 2026): the model sees Brevi's refusal next to
  the test result. Publish first-attempt native, retries per task, tokens-to-green and cost against CPython.
- The CPython control with the same native-code prompt line (a flag on the python arm).
- Auto-apply `fix`/`patch` on failure in the MCP server; return only the failing line, not the whole error set.
- Model-agnostic runs: one OpenAI-family, one Google-family and one open-weight model at minimum.

*Done when:* tokens-to-green ≤1.10x CPython at 90 days and ≤1.00x at 12 months, published; first-attempt native
published per release; one non-Claude model family above the release bar.

### WS5: Distribution, in the image

Adoption is about zero because installing needs `rustc` and no harness bundles Brevi.

- **SDK** (Python first, then Node): `brevi.run(code, inputs=..., limits=..., sandbox=True)`; the harness never
  shells out.
- **`python3` shim:** `brevi-python` behaves like `python3`, builds natively when safe and falls back silently.
- **Base images:** published, preinstalled images for E2B, Daytona, Modal, Cloudflare Sandbox and plain Docker
  (the templates in `templates/` are the start), offered upstream.
- A GitHub Action and a devcontainer feature for CI agents.
- **Hosted (`brevi.run`):** a closed beta for a handful of harness teams, only after WS7.
- Windows parity for library programs, and macOS `--standalone`.

Design: [SDK_AND_IMAGES.md](SDK_AND_IMAGES.md).
*Done when:* zero to a native run in ≤60 s with no Rust installed; one third-party platform ships Brevi in a base
image by month 6; the SDK has a 10-line example and an integration test in CI.

### WS6: Evidence that convinces a platform

- A real-task benchmark (a SWE-bench Verified or Terminal-Bench subset) with and without Brevi: success, wall-clock,
  tokens and $ per task.
- Model-agnostic runs and a per-release dashboard.
- One independent reproduction of the release test (raw logs and a run budget offered).
- No vanity metrics: the "400/400 random programs" claim gets a published generator or is removed.

### WS7: Safety at scale (gates the hosted service)

- External security review before `--sandbox` is marketed for hostile code (deferred until adoption and funding).
- OS-level isolation for hosted and recommended local use: the platform's VM or microVM, optionally gVisor or
  Firecracker, with the in-process capability check as a second layer.
- Wider Landlock coverage; on macOS and Windows, state the gap or refuse.
- Fuzz the capability check and the bridges; any escape is a security release.
- Sandbox claims on the site match `docs/THREAT_MODEL.md` exactly.

## Phases and gates

**Phase 0, weeks 1 to 4: instrument and tell the truth.** Conformance in CI and a public dashboard; the claims audit;
latency and first-attempt instrumentation; the fuzz harness; the verify-by-default design.
*Gate:* CI gate live, dashboard public, claims consistent, check latency measured, 10 alpha users recruited.

**Phase 1, weeks 5 to 12: loop, trust, distribution.** Verify-by-default prototype; daemon and `check --fast` as the
agent default; SDK and `python3` shim; two base images; real-task benchmark v1; a non-Claude model run.
*Gate (private alpha):* container install ≤60 s, check p50 ≤50 ms, verify-by-default usable, conformance ≥65%, no
panics in 30 days of fuzzing, 10 alpha users running weekly.

**Phase 2, weeks 13 to 24: fast path and hosted.** Hybrid typing; typed rate ≥80%; hosted closed beta; Windows library
parity; three model families; the first third-party integration.
*Gate (public beta):* conformance ≥75% and published, no absolute claims, external audit scheduled or its scope
published, two harnesses, hosted beta behind isolation, tokens-to-green ≤1.10x.

**Phase 3, months 7 to 12: 1.0.** Conformance ≥90% and no known-wrong results; typed rate ≥90%; check p50 ≤30 ms;
a stable 1.0 API; external audit complete; three harnesses; the real-task benchmark shows a win on success, latency
or cost; a support model (Brevi Pro or a hosted SLA) and a second human owning the semantic surface.

## The first 30 days

| Week | Item | Owner thread | Status |
|---|---|---|---|
| 1 | Conformance in CI with an allowlist gate | Fix the issues from the adversarial review | in progress |
| 1 | Claims audit: one source of truth, no absolutes, measured speed ranges | Fix the issues from the adversarial review | in progress |
| 1 | `bench/latency.py`; first-attempt metrics in the release test | Make Brevi the default runtime for agents | done 9 Nov |
| 1 | Recruit 10 alpha users | Adam (outreach drafted) | waiting |
| 2 | Fuzz harness; triage the 5 crashes and 2 signals | Fix the issues from the adversarial review | planned |
| 2 | Fix the top 5 known-wrong entries; publish the burn-down | Fix the issues from the adversarial review | planned |
| 2 | Verify-by-default design | Make Brevi the default runtime for agents | in progress |
| 3 | `check --fast` baseline; daemon spike; dev-backend go/no-go criteria | Make Brevi the default runtime for agents | baseline done 9 Nov |
| 3 | SDK and `python3` shim design; E2B and Daytona image recipe | Make Brevi the default runtime for agents | planned |
| 4 | Private alpha with opt-in traffic metrics (no telemetry by default) | Make Brevi the default runtime for agents | planned |
| 4 | Real-task benchmark v1 plan and first run | not started | planned |
| 4 | Public metric dashboard | Fix the issues from the adversarial review | planned |
| 4 | Conversations with two harness vendors and two sandbox platforms | Adam (outreach drafted) | waiting |

## Stop doing until 1.0

- New byte-parity library ports. Maintain what exists; bridge the rest.
- New `.bv` language features and the constrained-decoding grammar.
- GUI and tkinter work beyond bug fixes.
- Benchmark programs that flatter the headline.
- Absolute correctness language anywhere.

## Risks

| Risk | Likelihood | Impact | Mitigation | Trigger |
|---|---|---|---|---|
| Silent divergence found in the wild | High | Severe | Verify by default, conformance gate, fast fix release | Any report: P0 |
| Check latency target missed | Medium | High | Daemon first; dev-backend spike; keep `--fast` | full check p50 > 80 ms at week 8 |
| Hosted service abused or breached | Medium | Severe | VM isolation, audit, limits, abuse detection | Launch gate |
| Library treadmill | High | High | The stop-doing list; ports frozen | Any new port request |
| Key-person burnout | Medium | High | Second human on semantics; runbooks | Missed releases |
| CPython version drift | High | Medium | Pinned conformance target; CI on newest and next | Each Python release |
| Model behaviour changes | Medium | Medium | Multi-model runs each release; error-format tests | New model release |
| A platform forks Brevi (Apache-2.0) | Low | Medium | Move fast on integration; hosted value; brand | A platform ships its own |
| Benchmark credibility attack | High | Medium | Real-task benchmark, independent reproduction, artifact links | Public launch |

*This is a living document. The metric table is updated every release; a roadmap whose numbers do not move is a wish
list.*

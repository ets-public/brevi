# Brevi post-mortem

**Brevi is archived.** We built a Python-to-native compiler with Claude Code in 20 days. Then we tested it on the 10 most downloaded Python MCP servers on PyPI, and none of them built natively. We stopped there. This page says what we built, what worked, what didn't, and why we stopped. Every number comes with its source, its date, and what it does and doesn't cover.

Written 19 November 2026, at `main` 68e2f26 (just after v0.28.3).

## The short version

- **What it was:** a compiler that took plain Python, compiled it to Rust, and built a native binary with rustc. It had two build paths: a typed path for programs whose types stay fixed, and a dynamic path for everything else. It also had a CPython bridge for libraries it couldn't build, a sandbox, an MCP server, and `brevi run --verify`, which checked each run against CPython 3.13.
- **Size and pace:** the first commit was on 30 October 2026, and the last release was v0.28.3 on 19 November. In those 20 days we made 21 GitHub releases (v0.18.0 to v0.28.3), merged 592 pull requests, and wrote 257,236 lines under `src/`. Of those lines, 162,747 are Rust, 49,541 are Python (almost all of it copies of Python libraries), and 44,948 are generated data tables (Unicode names and interned strings). `tests/` holds another 60,592 lines. Almost all of the code was written by Claude Code (Opus, Sonnet and Haiku) and DeepSeek, working in parallel threads, with one person directing.
- **The result that ended it:** on 19 November we built the top 10 Python MCP servers by PyPI downloads without changing them. **0 of 10 built natively.** All 10 built only through the CPython bridge, so they still need Python on the machine and start in 1.00x to 1.16x CPython's time. `--standalone` produced binaries that broke at run time for 4 of the 10. The full table is in [bench/showcase/REAL_SERVERS.md](bench/showcase/REAL_SERVERS.md) (PR #688).
- **Adoption:** 0 GitHub stars and 0 forks on 19 November. We never found a user with a problem that Brevi solved better than what they already had.

## What we tried, in order

The project changed direction four times in 20 days. Each change had a reason, but each one also threw away part of the reason for the one before it.

1. **An AI-only language (30 Oct, first called Axon, renamed Brevi on 31 Oct).** The goal was a language based on Rust that models could write with fewer tokens than Python, counting both the code and the error messages they read. It never clearly beat Python. On the 18 small tasks at v0.8, Sonnet used 4,309 tokens in Brevi against 4,021 in Python, while Haiku used fewer (4,987 against 5,266). On the six XL tasks, Brevi started 8% (Sonnet) and 13% (Haiku) behind Python, and two fresh held-out sets at v0.18 and v0.19 still put it behind ([bench/HISTORY.md](bench/HISTORY.md)). Models already write Python well, so a new syntax had little room to win.
2. **Plain Python in, native binary out (v0.24, 3 Nov).** Brevi took ordinary Python and compiled it. This turned the project into a race to match CPython's behaviour, measured on CPython 3.13's own test suite.
3. **"The verified native runtime for agent-written Python" (16 Nov).** The pitch moved from speed to trust: each run is checked against CPython. On the core slice at v0.28.2, 97.3% of the scored suite matched CPython, with 83 string-as-code tests excluded (1,043 of 1,072), and none gave a different answer. But only 2.6% of those matches took the typed fast path. The typed build ran a median of 9.3x faster than CPython on 12 compute benchmarks, while the dynamic build, which most programs take, ran at a median of 0.85x (range 0.5x to 1.4x, 2026-11-12, Ryzen 9 5950X, Brevi 0.27.1; see [docs/NUMBERS.md](docs/NUMBERS.md)). So most programs got CPython's speed or worse, plus a rustc build step. There was no token saving either: in the weekly benchmark on 16 November (w1116, 24 tasks with 2 runs each, so 48 task-runs per model), agents writing plain Python for Brevi cost 1.075x what they cost writing it for CPython with Sonnet (range 1.04-1.12x), and 1.002x with Haiku (0.94-1.07x).
4. **A bundler for MCP servers and CLI scripts (Option A, 19 Nov).** After a second adversarial review, we narrowed Brevi to one job: turn a small Python MCP server or CLI script into one fast-starting native binary. Our own showcases looked strong. For example, time_mcp reached its first tool call in 35 ms against CPython's 730 ms (Brevi 0.26.0, Xeon 2.1 GHz with 4 cores, median of 10 runs, 2026-11-10). That was a native build: a bridged build starts within 1.16x of CPython's time, so it can't answer in 35 ms. The showcase's `needs_python: no` check uses `ldd`, though, and `ldd` can't see a bridged binary loading libpython at run time, so that check alone doesn't prove it. But those showcases were programs we had written or chosen ourselves. The real-server test was the first time we tried code we hadn't picked.

## The real-server test (A9, 19 November)

- **Corpus:** the top 10 Python MCP servers by pypistats downloads over the last month: mcp-atlassian, mcp-server-fetch, mcp-server-time, mcp-server-git, mcp-proxy, aws-api, aws-documentation, chroma-mcp, mcp-clickhouse and workspace-mcp. We left out mcp-grafana, which is a Go binary, and postgres-mcp, which can't be installed on CPython. We did not filter the list for what Brevi supports.
- **Result:** 0 of 10 built natively, and 10 of 10 built only through the bridge.
- **Even our showcase regressed:** the current mcp-server-time (2026.11.18) no longer builds natively. The 35 ms figure only holds for the pinned 2026.11.4.
- **Why they failed:**
  - 6 of 10 use fastmcp or `mcp.server.fastmcp`, and Brevi had no copy of either.
  - 5 of 10 need asyncio features beyond Brevi's own asyncio.
  - Several hit a typed-build aliasing refusal on ordinary settings attributes.
  - `--standalone` missed lazy imports, so those binaries broke at run time.
  - **Even with fastmcp and all of asyncio,** 8 of the 10 also import a large third-party library that Brevi has no copy of. These are boto3 and awscli, chromadb, clickhouse-connect, GitPython, atlassian-python-api, google-api-python-client, or the HTML stack of BeautifulSoup and markdownify. Only mcp-server-time and mcp-proxy don't. Copying those libraries was never realistic, so the servers would still have needed the bridge.
- **What it would have taken to fix:** copy fastmcp and more of asyncio and pydantic into Brevi, then keep those copies current as the libraries change. That is the same endless library-copying work we had just decided to stop.

## What worked

These are the parts we would keep and reuse on another project.

- **Honesty tooling.**
  - `tools/published_numbers.py` writes [docs/NUMBERS.md](docs/NUMBERS.md) from the release tag's own reports.
  - CI refuses any page that quotes the coverage share without its exclusions, or the typed speed without the dynamic one.
  - Conformance runs as a ratchet in CI, with zero wrong answers as a gate.
  - These rules came from the first adversarial review (9 Nov). That review found the site claiming "prints exactly what CPython prints" when the measured rate was 58.2% with 44 wrong answers.
- **Taking reviews seriously.** We had three outside reviews (9, 10 and 19 Nov). Each one found real problems, and we checked each claim against the code before acting on it. The 19 November review flagged undefined behaviour in v0.28.3's typed-build object handles (AxRow): `&'static mut` references that alias. A work-in-progress branch, `47y27x`, was already replacing them. We reproduced a use-after-free, a wrong answer and a stale read. On that branch, Miri went from 3 UB to 0 on about 150 typed programs.
- **Security process.**
  - 10 fuzz targets, with crash inputs kept as regression tests.
  - A public record of security testing in [docs/SECURITY_TESTING.md](docs/SECURITY_TESTING.md).
  - Model-written exploit code ran only in a sandbox (`tools/modelbox.py`).
  - Every security fix shipped at once in its own point release. v0.28.3 carried 8 fixes, including a `--sandbox` import escape, seccomp gaps, build-cache and CARGO_HOME trust, and MCP HTTP denial of service and DNS rebinding.
  - We published 5 GitHub security advisories.
  - We said plainly that Brevi had no external audit and that the sandbox is a policy layer, not an isolation boundary.
- **Release engineering.**
  - 21 releases in 20 days, with installers for Linux (x86-64 and ARM64), macOS and Windows.
  - Packages on PyPI, npm, Homebrew, Docker and a GitHub Action.
  - A pinned Rust toolchain fetched on first build.
  - Changelog fragments and generated release notes.
- **Working with a fleet of agents.** One thread per pull request, build and landing as separate threads, handoff notes under 30 lines, project memory, slow CI moved to a home PC, and cheap models for mechanical work. This let one person keep many threads moving at once.

## What didn't work

- **We built first and checked demand last.** We spent 20 days building before we ran the one test that mattered for the final pitch, which was real servers we didn't choose. That test took one afternoon. It should have come first, before each change of direction.
- **Agent speed hid the cost.** At about 13,000 lines a day, every new idea looked cheap to try, so we tried another direction instead of finding out whether anyone wanted the last one. The code was cheap to write, but expensive to keep correct: AxRow undefined behaviour, leaks in long-running processes, and 49,185 lines of library copies that each fell behind the real libraries.
- **Copying libraries never ends.** Real Python code depends on its libraries more than on the language itself. Every copy we made (pydantic, asyncio, httpx, the MCP SDK, numpy routes, polars and others) only covered what our own tests used. The first real servers needed parts we didn't have.
- **The fast path was too narrow.** The typed build was fast but took only 2.6% of conformance matches, and only 1.2% on the standard-library slice. Real server code has values that change type, and it goes to the dynamic path or the bridge.
- **Metrics we could hit instead of metrics that mattered.** CPython's own test suite was a good correctness check, but it was a poor measure of whether anyone could use Brevi. We spent days going from about 80% to 97%. A real-world corpus would have shown us the gap in hours.
- **No users.** 0 stars, a waitlist page with no backend behind it, and no conversation with an MCP server author. Compare that with this account's poe2-upgrade-checker, which solved one concrete problem for real players and got stars and forks.

## Why we stopped

The bundler was the best direction we had left, and it failed its first real test with 0 of 10 servers. Getting those servers to build natively would have meant copying fastmcp and much more of asyncio and pydantic, and large third-party SDKs like boto3, chromadb and GitPython, and then keeping up with every new release of each. That is the same treadmill we had stopped on 19 November. We had spent only 20 days, so it was cheap to stop. Changing direction a fifth time would have cost more than writing this up honestly.

## State at archive

- **main:** before archiving, it gets the AxRow aliasing-UB fix (from branch `47y27x`), the real-server report (#688), and an "experimental, archived" notice on the site.
- **Known runtime limits we did not fix:**
  - the `rt_gen` stack-switching assembly doesn't save floating-point control state and has no unwind info;
  - `AX_ASCII` caches pointer, length and capacity in relaxed atomics;
  - `alloc_rt` never returns small-object pages;
  - `rt_gc` only collects cycles reachable from a weakref;
  - every AxRow is leaked by design.

  None of these are in [docs/KNOWN_ISSUES.md](docs/KNOWN_ISSUES.md), so this list is the record of them.
- **Open pull requests:** closed, each with a note.
- **Releases:** everything up to v0.28.3 stays downloadable. There will be no further releases or security fixes. Do not use Brevi for anything that matters.
- **Reuse:** the code is Apache-2.0. Parts that could be useful on their own:
  - the conformance harness (`tools/conformance.py`)
  - the published-numbers check (`tools/published_numbers.py`)
  - the fuzz targets (`fuzz/`)
  - the Python-to-Rust emitter, as a case study

## Lessons

1. Test on real-world inputs that someone else picked, before choosing a direction and again before each change.
2. When code is cheap to write, demand is what limits you. Talk to five users before writing the first 10,000 lines.
3. "Runs your Python" means running your libraries too. Any plan that depends on copying a library has to price in keeping that copy current.
4. Honesty tooling is worth building early. It made this write-up easy to check, and it caught us overclaiming more than once.
5. Adversarial reviews are cheap, so run them early and often.

# Security testing

This page records what has been done to test Brevi's security, how to repeat it, and what was found. It complements [THREAT_MODEL.md](THREAT_MODEL.md), which says what the sandbox, the limits and `brevi mcp` promise and gives the attack-surface table this page tests against. To report a vulnerability, see [SECURITY.md](../SECURITY.md).

**Brevi has had no external security audit.** Everything below is the project testing itself. That catches a lot, but it is no substitute for an independent review. Until there has been one, run code you have reason to distrust inside a container or VM, and treat `--allow-py` and `--allow native` as full trust.

## The programme

| Part | What | Where it runs | Status |
| --- | --- | --- | --- |
| Threat model | the attack surface, one row per place outside input reaches Brevi | [THREAT_MODEL.md](THREAT_MODEL.md#attack-surface) | done 2026-11-16 |
| Coverage-guided fuzzing | 10 `cargo fuzz` targets over the surfaces, with AddressSanitizer and debug assertions on | maintainer's machine (`tools/security_checks.sh`) | first runs 2026-11-16 (below) |
| Program fuzzing | generated Python programs built both ways and compared with CPython (`tools/fuzz.py`, `bytes_fuzz.py`, `trace_fuzz.py`) | GitHub, daily and on every PR (saved corpus) | in place |
| Static analysis | CodeQL (`security-extended`) on Rust, Python and the workflows | GitHub, weekly (`.github/workflows/codeql.yml`) | from 2026-11-16 |
| Dependencies | `cargo audit` and `cargo deny` (`deny.toml`) on every Cargo.lock; OSV advisories for every dependency three times a week (`tools/advisories.py`); Dependabot | maintainer's machine; GitHub (`upstream.yml`) | audit/deny from 2026-11-16 |
| Unsafe code | `cargo geiger` on the compiler and `tools/unsafe_inventory.py`, which also counts the runtime code pasted into programs | maintainer's machine | 862 `unsafe` uses on 2026-11-16, 820 of them in runtime code (FFI glue for CPython, DuckDB, ONNX Runtime, SQLite, the network) |
| Sanitizers | the compiler's unit tests under AddressSanitizer (`-Zsanitizer=address`, `-Zbuild-std`) | maintainer's machine | script ready; first run pending |
| Windows | Application Verifier and PageHeap on built programs; the planted-DLL attempts below | maintainer's Windows machine | planned |
| Supply chain | actions pinned by SHA (`tools/workflow_guard.py`); SBOM, build provenance and a reproducible-build check on every release; engine libraries pinned by SHA-256; Rust pinned in CI; fuzz dependencies pinned in `fuzz/Cargo.lock` | GitHub | in place |
| Self red-team | attempts to break each promise, run inside `tools/modelbox.py` | cloud sandbox, never the maintainer's machine | planned (list below) |
| External review | a scoped design review by volunteer reviewers (DLL loading, Job Objects, MCP), credited | — | being arranged |
| OSS-Fuzz | application draft in `fuzz/oss-fuzz/` | Google's infrastructure, if accepted | draft |

GitHub Actions minutes are limited, so heavy runs (long fuzzing, sanitizer builds, cargo audit, deny and geiger) run on the maintainer's machine with `sh tools/security_checks.sh [SECONDS_PER_TARGET]`. The script writes `security-checks/summary.txt` with PASS, FAIL or SKIP for each step. A missing tool is a SKIP, never a PASS.

## Fuzz targets

`cargo +nightly fuzz run TARGET` from the repository root (needs `cargo install cargo-fuzz`). The fuzz package (`fuzz/`) builds the compiler as a library by pasting in `src/main.rs`, so the targets run Brevi's own code. Runtime code that Brevi pastes into programs (the folder check, the CPython bundle reader) is built in from the same source files. Nothing in `fuzz/` ships, and the compiler still has no dependencies.

| Target | Surface (THREAT_MODEL row) | Input | Property checked besides "no panic, no crash, no hang" |
| --- | --- | --- | --- |
| `parse_bv` | 1 source text | any text | — (lexer and parser) |
| `parse_py` | 1 source text | any text | — (the dynamic build's parser and f-string parser) |
| `caps_check` | 2 capability check | any text | — (a panic there would be a way around the check) |
| `mcp_json` | 4 MCP JSON-RPC | any text | what the reader accepts, the writer writes back as JSON the reader accepts and writes the same way again; every tool's argument check |
| `mcp_grants` | 4 MCP grants | a tool call's arguments, under five server policies | without `--trust-calls` a call never runs unsandboxed, and every grant it passes on is inside the server's flags, checked again from the flags with paths resolved on disk (the granted folder holds links that lead out of it) |
| `mcp_http` | 5 MCP over HTTP | raw request bytes | a body is never over the 4 MiB cap; header names are lower-cased |
| `limits` | 6 resource limits | a `--limit` value | a value that parses is written back (as passed to the child process) as one that parses to the same limits, none dropped and none looser |
| `fs_scope` | 7 folder grants | a path, relative or absolute | a path the check allows resolves inside the granted folder, which holds links to outside, a dangling link and a link loop |
| `bundle_inflate` | 9 CPython bundle | DEFLATE data | never more output than asked for |
| `bundle_unpack` | 9 CPython bundle | a bundle | nothing is written outside the folder it unpacks into |

A crash that has been fixed stays fixed: its input is kept in `fuzz/regressions/TARGET/`. Run a target on that folder to replay them (`fuzz/target/*/release/TARGET fuzz/regressions/TARGET/*`).

### Results

**2026-11-16, first runs.** 2 minutes per target, two targets at a time on a 4-core cloud container, with ASan and debug assertions. Seeds for the parsers were 300 Python files and 200 `.bv` files from `tests/`. These are smoke runs to catch shallow bugs. Long runs on the maintainer's machine come next.

| Target | Runs | Coverage (edges) | Found |
| --- | --- | --- | --- |
| `parse_bv` | 54,805 | 15,161 | nothing |
| `parse_py` | 26,922 | 3,546 | nothing |
| `caps_check` | 26,514 | 14,411 | nothing |
| `mcp_json` | — | 507 | **a bug, fixed:** a number beyond f64's range (`1e999`) was read as infinity and written back as `inf`, which is not JSON. The reader now refuses it. It had no security impact: the worst case was an invalid reply to the client that sent it. |
| `mcp_grants` | 7,487,332 | 1,142 | nothing |
| `mcp_http` | 2,204,603 | 726 | nothing in this run. A finding in the same code from review is listed under "Findings" (fixed in v0.28.2). |
| `limits` | 3,426,075 | 761 | nothing |
| `fs_scope` | 4,685,796 | 165 | nothing |
| `bundle_inflate` | 721,420 | 246 | nothing |
| `bundle_unpack` | — | 377 | **a bug, fixed:** a file time past what `SystemTime` holds panicked. It had no security impact: the bundle is the program's own trailer, and changing it means changing the program. |

## Self red-team

Each attempt tries to break one promise in THREAT_MODEL.md. Attempts are written down here whether they succeed or fail. Exploit code is model-written, so it runs only inside `tools/modelbox.py`, never on a maintainer's machine. A successful attempt is a vulnerability: it is fixed in its own security point release, as [SECURITY.md](../SECURITY.md) describes, and recorded here once the fix ships.

| # | Attempt | Promise tested | Status |
| --- | --- | --- | --- |
| R1 | Planted DLL: a DLL with the name of a dependency of DuckDB, ONNX Runtime, OpenSSL or CPython, placed in the program's folder, the working folder and a `PATH` folder | Windows libraries load only from their own folder or System32 | planned (Windows) |
| R2 | Swap a pinned engine library between the hash check and the load | pinned libraries | planned |
| R3 | MCP grant bypass: calls that widen grants (duplicate keys, odd types, paths with `..` and links, case tricks on hosts, module names with dots) | a call never gets more than the server allows | partly covered by `mcp_grants` fuzzing; hand-written attempts planned |
| R4 | Sandbox escape corpus: Python that tries to reach files, the network, the environment or processes without a grant, through every module Brevi builds natively | refusals are final; the dynamic runtime reaches nothing | planned (kept as a test corpus) |
| R5 | Handle and memory exhaustion: fds, threads, child processes, memory maps, huge allocations, under each limit | limits hold from the first instruction | planned |
| R6 | Limit bypass: work in a child process, in threads, through CPython, around `written`, `files` and `net` | limits hold where documented, are refused elsewhere | planned |
| R7 | Bridge crash: crash CPython through the bridge and check that the program fails cleanly without escaping its holds | Landlock and seccomp hold every thread | planned |
| R8 | `brevi mcp --http` from a web page (DNS rebinding, missing Origin, a token sent to the wrong host) | only the token holder or the local machine reaches the server | planned |

### Findings

A finding is listed with its details once a fixed release has shipped. Before that, only its existence is listed, so nothing is disclosed before users can upgrade. Each fix ships in its own security point release, with a GitHub security advisory.

| Found | How | Finding | Affected | Fixed |
| --- | --- | --- | --- | --- |
| 2026-11-16 | code review | Temporary folders (build folders, cargo projects, MCP work folders, background build slots, a `--standalone` binary's CPython with no home folder) had fixed names in the shared temp folder, so another local user on a unix machine could create them first and read or change what Brevi built or ran. Now every one lives in `$TMPDIR/brevi-UID` (else `~/.cache/brevi/tmp`), used only when it is owned by the user, private, and under parents no one else can change. Windows was not affected. | v0.28.1 and earlier, Linux, macOS and other unix | v0.28.2 (#602) |
| 2026-11-16 | fuzzing and code review | `brevi mcp --http` read a request before checking its token with no bound on chunked body size, time or connections, so a client without the token could make it hold a lot of memory or keep connections open (denial of service; no code execution, no token bypass). Now a body past 4 MiB gets 413, a silent or slow request is closed (30 s idle, 120 s whole), and at most 64 connections are served. stdio `brevi mcp` was not affected. | v0.26.0 to v0.28.1, only with `--http` | v0.28.2 (#603) |

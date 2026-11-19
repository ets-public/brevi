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

## Shipped recently

Each item is in the [changelog](/docs/changelog) with its release.

- **In v0.28.3:** security fixes: every parsed import checked under the sandbox, the child-process code checks the native grant and Brevi's internal modules cannot be imported (a `--sandbox` escape closed); unknown options refused; seccomp keeps sandboxed CPython from other processes on Linux; sandboxed CPython refused where a kernel lock is missing unless `--allow-unlocked-python`; no grant reaches Brevi's caches and no shared cache or cargo home is trusted; `brevi mcp --http` backs off wrong tokens; crate lines checked; release supply chain pinned.
- **In v0.28.2:** security fixes for temp folders (private to the user on Linux and other unix) and `brevi mcp --http` (requests bounded before the token check); on Windows, CRLF source, source paths without `\\?\`, the cp1252 codec and `re.LOCALE`, and the standard-library conformance slice gates a release.
- **In v0.28.1:** `re.IGNORECASE` and str case methods answer with CPython 3.13's Unicode tables on any Rust toolchain (Rust 1.100 pinned); `namedtuple` names checked as CPython checks them; run-time regex patterns the typed matcher lacks re-run on the dynamic build; Windows `--standalone`, the Windows certificate store, Windows ARM64, and Windows conformance as a release gate.
- **In v0.28.0:** `--strict`; core conformance 97.3% (of the scored suite, with 83 string-as-code tests excluded) and stdlib 96.9% over 21 files with no wrong answers; `match` statements; complex numbers, metaclasses, `def f[T]`, threads that take turns, `weakref.proxy`, strs with lone surrogates, and exact `vars()`/`dir()` in dynamic builds; releases held to the conformance gates, with an SBOM, provenance attestations and a reproducible-build check; native `bytearray`, `memoryview`, `collections.abc`, `ChainMap` and the `User*` classes, `functools.singledispatch`, `partialmethod` and `cached_property`, `string.Template`, `itertools.tee` and `batched`, warnings with the program's lines, and bytes patterns in `re`; sandboxed CPython gets no network without a grant; engine libraries pinned by hash; Windows paths as `ntpath`.
- **v0.25.0 and earlier:** the sandbox on every build path, caller-only grants, native numpy, the dynamic build for programs the typed build refuses, and `pip install brevi`, `npx brevi`, `cargo install brevi` and the MCP registry.

## Always


# Roadmap
<!-- description: What is next for Brevi: the public preview, package managers, brevi.run, and the compiler work in progress. -->

Brevi's direction stays the same: **AI-first**, with speed and safety as the reasons an agent should run its Python through Brevi. Token savings are added step by step and each one is measured in the release test before it counts. This page lists what is planned, not promised dates.

## Now

- **Public preview on GitHub.** The source, the releases and these docs, under Apache-2.0.
- **Sandbox on every path** (v0.24.1). The `allow` check runs before every build path, Python libraries need `allow native`, and a sandboxed program never falls back to CPython.
- **The standard library, natively.** Standard-library calls no longer need CPython: what the typed build lacks runs in the native dynamic build (v0.24.3), and the common ones move onto the typed fast path module by module (`math` and `time` first). In v0.24.4: `enum` (members, `auto()`, `@unique`, `str` and `int` mixins), `json.JSONDecoder().raw_decode`, `io.StringIO`, path text from `os` and `pathlib` outside the sandbox, and `bin`, `hex`, `oct` and `fractions.Fraction` on the typed path. Next: `random` outside the sandbox, `textwrap`, `csv.writer`, `datetime.strptime`, `IntEnum` and `Flag`, bytes (`hashlib`, `base64`), then `json` indent and `dataclasses.asdict` on the typed path. The [table](/docs/compatibility#what-runs-native) shows where each function runs.
- **Cheaper heavy tasks.** Tell agents their Python is compiled so they write the plain solution, and make plain idioms (generators, string indexing) build typed and fast.

## Next

- **Package managers.** `pip install brevi`, `npx brevi mcp`, `brew install`, `cargo install brevi` and the MCP registry. The packaging is ready.
- **brevi.run.** The compiler as a hosted API for agents: check, run and test over HTTPS, sandboxed, with cached builds. Then a playground in the browser.
- **More programs on the fast path.** Widen what the typed build accepts, so fewer programs need the slower dynamic build, and let library programs the typed build rejects still build natively.
- **More of Python natively.** `match` statements, `async`, and more standard modules in dynamic builds.

## Later

- **Windows parity.** Library calls and `--standalone` on Windows; `--standalone` on macOS.
- **Faster cold builds**, for agents that build many one-off programs.
- **Larger and fresher benchmarks**, run weekly on new model versions, with results published here.

## Always

- Every program that once passed must keep passing: the full set of saved solutions is rerun on every change and on every new Rust and Python release.
- Brevi matches CPython or refuses. A silent difference is treated as the worst kind of bug.

Changes ship in releases listed in the [changelog](/docs/changelog).

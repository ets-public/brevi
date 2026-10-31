# Changelog

Each release's section here becomes its GitHub release notes (`.github/workflows/release.yml`; `release-notes.yml` re-syncs the notes of an existing release). Benchmark figures are tokens per finished task (code written plus errors read, retries scored as patches), from [bench/LEADERBOARD.md](bench/LEADERBOARD.md).

## Unreleased

- Python 3.15 unpacking in comprehensions (`[*xs for xs in xss]`) reads as a comprehension followed by `flatten()` (#15). Rust 1.99 builds cleanly (#13).
- The core guide is trimmed from 5,035 to 3,490 tokens; rare details move to REFERENCE.md, served by `brevi guide WORD`, and CI fails when BREVI.md goes over 3,600 tokens (#18).
- Install channels prepared for launch, not yet published: Claude Code plugin and marketplace, MCP registry `server.json`, a PyPI wrapper, an npm wrapper (`brevi-lang`) and a dataset export of 49 tasks with 1,893 verified solutions (#20).
- A second held-out set of six XL tasks (`bench/ref/gen_big4.py`). On it Brevi is behind Python: Sonnet 10,997 vs 9,222 tokens, Haiku 11,851 vs 11,426 (#19).

## v0.18.0 (31 Oct 2026)

Closes most of the gap on the six XL tasks (150 to 400 lines, two split over several files): Sonnet 12,630 vs Python 13,128 tokens (4% fewer), Haiku 15,908 vs 15,658 (2% more), from 8% and 13% behind. Brevi was tuned on these runs.

- Python's `sorted(xs, key=f, reverse=True)`, `min(xs, key=f)`, `lambda`, format specs such as `{x:.2f}`, and `print(x, end="", sep=", ")`.
- `fail` used like Python's `raise`, with one `match` at the top.
- Rows and nested fields edited in place (`for r in rows: r[i] = v`, `books[id].wait.push(x)`), also while the loop reads the same table.
- Global maps filled through a copy inside a function; a function's own local may share a top-level name.
- `return 0` in a function that otherwise ends on a print; `while True:` with a return inside; `(_, x) =` in branches; names reused across sibling blocks; `n.range()`.
- Prebuilt binaries for Linux (x86-64, ARM), macOS (Intel, Apple silicon) and Windows, downloaded by `install.sh`.

## v0.17 (31 Oct 2026, no separate release)

- Libraries: `use "util.bv"` for multi-file programs, `use sha2 "0.10"` with `rust fn` bodies for Rust crates, and `py fn` with Python bodies for any installed Python package. See [LIBRARIES.md](LIBRARIES.md).
- Sonnet passes first time on 22 of 24 big tasks (39 of 48 in v0.15); Brevi is ahead of Python on the eight big tasks for both models (Sonnet 8,284 vs 8,721, Haiku 9,398 vs 10,556).
- `"{{"` and `"}}"` as a method's first argument, `j.int()`/`j.float()` on JSON strings, `s.find(sub, start)`, `s.char_at(i)`, untyped parameters typed from their calls, and a program's own `fn min`, `fn args` or `type Ord`.
- The MCP `check` tool can return the fixed program (`fix: true`).
- Daily toolchain checks on Rust stable, beta and nightly and the newest and next Python, rerunning every benchmark solution that ever passed (#2).

## v0.16 and earlier

- v0.16: `brevi mcp` (local MCP server), `brevi check --fast`, `brevi patch`, `brevi guide WORD`, Agent Skills package, AGENTS.md, llms.txt, install.sh, Dockerfile.
- v0.11 to v0.15: Brevi's own type checker before rustc, exact token counts in the benchmark, template strings, Json reads.
- v0.4 to v0.10: the core language (then called Axon): Python-style blocks and spellings, implicit error passing, ownership inference, heaps, decimals.

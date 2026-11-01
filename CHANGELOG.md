# Changelog

Each release's section here becomes its GitHub release notes (`.github/workflows/release.yml`; `release-notes.yml` re-syncs the notes of an existing release). Benchmark figures are tokens per finished task (code written plus errors read, retries scored as patches), from [bench/LEADERBOARD.md](bench/LEADERBOARD.md).

## Unreleased

- Python's `f"..."` and `f'...'` strings are accepted (Brevi strings already fill in `{name}`).
- A program's own type named `Result` no longer clashes with Rust's.
- A top-level map filled inside a function (`ts = {}`, then `ts[n] = Team()` in a function) is typed where other functions read it, so `fn key(n: str) = (-ts[n].w, n)` needs no return type. This was the most common first-check error in recent runs.
- `inner = m.get_or(k, {})` is typed by what is done with it, like a fresh `{}`, so nested maps built this way at the top level work inside functions.
- A function ending in `if c: m.remove(k) else: print(...)` gives no value, as in Python.
- Functions are values: `ops = [add, mul]`, pairs such as `("+", add)`, and calls straight from a list, tuple or field (`ops[0](3, 4)`).
- `None` for int, float, str and tuple values: `best = None`, `x is None`, `x is not None`, `return None`.
- Maps of functions: `ops = {"+": add, "-": lambda a, b: a - b}`, then `ops[op](a, b)` or `for k, f in ops.items()`.
- More Python habits: `set(xs)` and `s.add(x)`, `xs.extend(ys)`, `xs[::-1]`, `divmod`, `round(x)`/`round(x, n)`, `enumerate(xs, 1)`, `m.setdefault(k, [])`, `defaultdict(int)`, `"%d %s" % (a, b)`, `"{:>5}".format(x)`, `math.sqrt`, `sys.stdin`, `[0] * n`, `p.0.1`, and `import sys`/`from collections import ...` lines. These came from compile errors in earlier benchmark runs.
- Python 3.15 unpacking in comprehensions (`[*xs for xs in xss]`) reads as a comprehension followed by `flatten()` (#15). Rust 1.99 builds cleanly (#13).
- The core guide is trimmed from 5,035 to 3,490 tokens; rare details move to REFERENCE.md, served by `brevi guide WORD`, and CI fails when BREVI.md goes over 3,600 tokens (#18).
- Install channels prepared for launch, not yet published: Claude Code plugin and marketplace, MCP registry `server.json`, a PyPI wrapper, an npm wrapper (`brevi-lang`) and a dataset export of 49 tasks with 1,893 verified solutions (#20).
- Grammar for constrained decoding: `grammar/brevi.lark` and a llama.cpp copy, `grammar/brevi.gbnf`, which accept every test program and all 1,370 verified Brevi solutions. Examples for LangChain, the OpenAI Agents SDK, CrewAI and the Vercel AI SDK over `brevi mcp` (`integrations/`), and a Homebrew formula generator (#22).
- Safe to run: `allow read, write` (or `native`) at the top of a program declares what it touches, `brevi run --sandbox` refuses anything undeclared, and the MCP tools sandbox by default (`sandbox: false` turns it off).
- A second held-out set of six XL tasks (`bench/ref/gen_big4.py`). On it Brevi is behind Python: Sonnet 10,997 vs 9,222 tokens, Haiku 11,851 vs 11,426 (#19).
- Faster builds: the runtime prelude is compiled once per rustc version into a cached library instead of being rebuilt with every program (a build goes from about 0.73 s to 0.36 s, `brevi check` from 0.55 s to 0.21 s), and builds are cached by a hash of the generated code, flags and rustc version, so running the same program again takes about 0.1 s. Programs are built without debug info (5 MB to 1.1 MB). The compatibility check of 1,971 old solutions drops from about 6 to under 3 minutes. `BREVI_NO_CACHE=1` and `BREVI_NO_RTLIB=1` turn these off.
- `brevi mcp` keeps the program between calls: send `code` once, then change it with `patch` (whole functions by name) or the new `edit` (a range of lines), which check again in the same reply, so a retry costs the changed lines instead of the whole file. Multi-file programs work through `file`. `run` takes `cases` (stdin and expected output), builds once, and prints `ok` or the first differing line per case. `show` prints the kept program with line numbers. Calls that pass `code` work as before.
- Sorting by a key (`xs.sort_by(f)`, `sorted(xs, key=f)`) works out each key once, as Python does, instead of twice per comparison. `--release` builds keep the prelude inside the program so its functions can be inlined.
- An end-to-end benchmark, `bench/e2e.py`, counts every token and dollar the API bills per finished task (guide, thinking, tool calls, retries). On held-out XL set 3 Brevi costs 58% more than Python with Sonnet and 78% more with Haiku, mostly in model output; see bench/README.md.

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

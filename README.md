<img src="site/icons/favicon.svg" alt="Brevi logo" width="72" height="72">

# Brevi

A programming language for AI models to write. Brevi compiles to Rust, so programs get Rust's speed and safety, but the model never writes lifetimes, borrows or semicolons, and every compiler error comes back as one short JSON line with a suggested fix.

The guide a model reads before writing Brevi is [BREVI.md](BREVI.md), 4,665 tokens (exact count, Sonnet tokenizer).

## For agents

- **Skill:** [skills/brevi/SKILL.md](skills/brevi/SKILL.md) is an Agent Skills package: setup, then the write, check, fix loop.
- **MCP server:** `brevi mcp` serves the tools `guide`, `check`, `run`, `test` and `patch` over stdio. Each takes the program as text, so the agent needs no files. Add it to a client as the command `brevi` with the argument `mcp`.
- **Guide on demand:** `brevi guide` prints the guide built into the compiler, `brevi guide WORD` only the lines about one name, and `--explain` on any command adds those lines under each error.
- **Fast check:** `brevi check f.bv --fast` runs Brevi's own checks in about 3 ms, without rustc (a full check takes about 400 ms).
- **Edit by name:** `brevi patch f.bv p.bv` replaces each `fn`, `type`, `enum` or `test` in p.bv by name, or adds it; `del fn name` removes one.
- **llms.txt:** `site/build.sh` builds llms.txt, llms-full.txt and .md pages for brevi-lang.org into site/out.
- **Install:** `sh install.sh` fetches the Rust toolchain if it is missing and builds brevi with `cargo install` (it needs access to this repo while it is private). The [Dockerfile](Dockerfile) bundles brevi with rustc: `docker run --rm -i brevi mcp`.
- **Leaderboard:** [bench/LEADERBOARD.md](bench/LEADERBOARD.md), built by `bench/leaderboard.py` from the logs.
- Working on the compiler itself: [AGENTS.md](AGENTS.md).

## Status: v0.16

Works today:
- Parser for the full core syntax in BREVI.md
- Front-end checks with short JSON errors: unknown names and methods with "did you mean" hints, wrong argument counts, names used outside their block
- Brevi's own type check before rustc: a name changing type, `str + int`, int/f64 mixing and wrong argument types get plain one-line errors with the line where the name was set. `t[1]` on a tuple, `xs.1` on a list and whole-number literals next to an f64 are fixed silently
- Emits Rust and builds it with `rustc`. rustc errors are mapped back to the Brevi line and rewritten in Brevi's type names
- `run`, `test`, `check`, `build`, `emit`, and `fix`, which writes every suggested rename into the file
- Errors are `{"line":N,"msg":...,"fix":...}`: no file path, column, kind or rustc code. A small typo of a known name (`totl` for `total`) is fixed during the build without an error
- Python spellings models write by habit are accepted: `True`/`False`, `a if c else b`, `range()`, `for c in s`, `enumerate()`, `int()`, `ord()`, `chr()`, `.append()`, `.strip()`, `.split()`, `.items()`, `.get(k, d)`, `", ".join(xs)` and more. `let` names can be changed, and `xs.sort()` as a statement sorts xs
- Runtime prelude with strings, lists, maps (hash lookups, listed in key order), files, stdin, JSON, CSV, dates, big integers, exact decimals and a min-heap
- JSON objects from map literals (`str({"id": 1, "name": n})`), `x.json()`, and Python-syntax regexes (`re_full`, `re_find`, `re_all`, `re_sub`, `re_split`)
- Python-style indented blocks (`if x:` and an indented body), with braces still accepted
- Empty lists, maps and heaps need no type: the compiler works it out from later use
- Short syntax: `x = 1` declares, `x: [str] = []` gives a type, `(a, b) = pair`, errors pass up without `?`, field defaults and shorthand constructors
- `.or(default)` and `match f(x) { Ok(v) => ... Err(m) => ... }` handle failures; runtime errors are one JSON line with the Brevi line
- Ownership inference: a variable's last read moves its value, keys, comparisons and indexes read in place, and loops read lists without copying them. If a move would break Rust's borrow rules, the compiler quietly builds the copying version instead
- Benchmarks in `bench/`: 8 bigger tasks (50 to 200 lines), 10 hard tasks, 8 held-out tasks written after the language was tuned, 4 edits to an existing program, and 4 speed tests, with results for Claude Sonnet and Haiku in Brevi, Python and Rust

Held-out check and full re-run, v0.16 (31 Oct 2026, exact tokens, retries scored as patches). Six new big tasks (`bench/ref/gen_big2.py`: sales_report, route_planner, ini_config, stack_vm, line_diff, room_booking) were written after v0.15 and never used to tune Brevi. Each cell is the average of two runs; the small and Haiku big tasks were run again on the v0.15 compiler.

| Task set | Model | Brevi | Python | Brevi saves | First try (Brevi / Python) |
| --- | --- | --- | --- | --- | --- |
| 6 held-out big | Sonnet | **4,020** | 4,220 | 5% | 11 of 12 / 12 of 12 |
| 6 held-out big | Haiku | **4,604** | 5,847 | 21% | 10 of 12 / 10 of 12 |
| 18 small | Sonnet | **4,488** | 5,053 | 11% | 36 of 36 / 35 of 36 |
| 18 small | Haiku | **5,306** | 5,795 | 8% | 34 of 36 / 36 of 36 |
| 8 big | Haiku | **10,382** | 10,556 | 2% | 11 of 16 / 45 of 48 |

Brevi is ahead on every set, including tasks it was never tuned on. The Haiku big-task margin is small because run p20 hit three compiler gaps; v0.16 fixes all of them, so those three programs now pass as written: a fallible call saved to a name and then matched with `Ok`/`Err`, a top-level `x = 0.dec()` used inside a function, and `e[1]` on a tuple taken from a list whose type comes from later pushes. v0.16 also lets a loop variable be changed in place (`for k, g in m.pairs(): g.sort()`), the one small-task failure, and accepts `Ok(x):` as a match arm. The held-out runs found four more gaps, fixed before the re-run: nested `m[a][b] = v` into an empty map, an empty value never used again, `_` in partial unpacking, and `d.year`/`d.weekday` on dates. All 889 old solutions print the same.

Sonnet on big tasks, v0.15 (31 Oct 2026, exact tokens, retries scored as patches): six fresh Brevi runs (p13 to p18) against all six Python runs.

| Task set | Model | Brevi | Python | Brevi finished code | Python finished code | First try (Brevi / Python) |
| --- | --- | --- | --- | --- | --- | --- |
| 8 big | Sonnet | 8,721 | 8,721 | **8,061** | 8,665 | 39 of 48 / 48 of 48 |

Sonnet is now level with Python on big tasks (the last three runs, on the final v0.15 compiler, average 8,623). Brevi's finished code is 7% shorter; the retries after first-try failures use up that saving. v0.15 fixes the failures seen in runs p9 to p15: template tags in double-quoted strings (`"{{#each "`, `"{{/if}}"`, `"{{else}}"`) keep both braces, `'single quotes'` work inside `{...}` values, a function that ends in an if or match needs no return type, and a match can be written inside brackets. Of the 15 Sonnet first-try failures in runs p3 to p12, 12 now pass as written. All 829 old solutions print the same.

Big tasks on v0.14 (31 Oct 2026, exact tokens, retries scored as patches). Brevi runs p9 to p12 on the v0.14 compiler; Python is all six of its runs:

| Task set | Model | Brevi | Python | Brevi finished code | Python finished code | First try (Brevi / Python) |
| --- | --- | --- | --- | --- | --- | --- |
| 8 big | Sonnet | 9,784 | **8,721** | **8,214** | 8,665 | 28 of 32 / 48 of 48 |
| 8 big | Haiku | **9,523** | 10,556 | **9,346** | 10,138 | 21 of 24 / 45 of 48 |

Haiku now uses 10% fewer tokens than Python on big tasks, with every Brevi run (9,180 to 9,890) below the Python average. Sonnet's finished Brevi code is 5% shorter than Python's, but Sonnet passes first time less often in Brevi, so its total is 12% higher; one Sonnet run (p11) also gave up on template after writing `"{{#each "` and expecting two braces (12,042 tokens). Without that run Sonnet is 9,032 vs 8,721. Two of the Sonnet failures are fixed in the compiler since (`str({...})` now keeps keys in written order; `'{{'` in single quotes is two braces).

v0.14 adds `'single-quoted'` strings (no `\"` needed, so JSON text is short), `s.re_list(p)` for tokenizers, `s.partition(sep)`, `_` in tuple unpacking, names set on only some paths starting at their type's zero (as Python code expects), and `str({...})` keeping written key order. All 828 old solutions print the same, and every recent Brevi solution that passed still passes.

Big tasks with more runs (v0.13, 31 Oct 2026, exact tokens, retries scored as patches). Six fresh Brevi runs (p3 to p8) and five Python runs (p1, p2, p5, p6, p7) per model:

| Task set | Model | Brevi | Python | Brevi range | Python range |
| --- | --- | --- | --- | --- | --- |
| 8 big | Sonnet | 9,179 | **8,663** | 8,677 to 9,936 | 8,233 to 9,303 |
| 8 big | Haiku | 10,931 | **10,507** | 10,143 to 12,233 | 9,099 to 11,612 |

With more runs the big-task picture is a loss, not a tie: Brevi uses about 6% (Sonnet) and 4% (Haiku) more tokens than Python. Most of the gap is now the finished code, not retries: Brevi is shorter on access_report and kv_store, but longer on template, spreadsheet, bank_api and invoices, which read JSON and build small parsers. v0.13 adds direct JSON reads (`j.text()`, `j.int()`, `j.dec()`, `j.is_null()`, `for x in j`, `j == "gold"`, Json keys), `.or()` after steps that cannot fail, conversions inside plain lambdas, and fixes for globals updated as `n = max(n, x)`, lambda names leaking out and `.json()` results. All 807 old solutions print the same and all 168 recent Brevi solutions still pass.

Exact tokens (v0.12, 31 Oct 2026), counted with Anthropic's count_tokens endpoint (Sonnet tokenizer). Each figure is code written plus errors read for the whole task set, averaged over 2 fresh runs; every task was solved in every run. Retries are counted as patches: the first check counts the whole file and each retry counts only the lines it changed (old plus new), the same for both languages, and subjects are told to edit only the lines that need it.

| Task set | Model | Brevi | Python | Brevi finished code | Python finished code | First try (Brevi / Python) |
| --- | --- | --- | --- | --- | --- | --- |
| 18 small | Sonnet | **4,654** | 5,016 | 4,487 | 4,890 | 35 / 36 of 36 |
| 18 small | Haiku | **5,307** | 5,671 | 5,125 | 5,545 | 34 / 36 of 36 |
| 8 big | Sonnet | **8,833** | 9,141 | 8,597 | 9,085 | 13 / 16 of 16 |
| 8 big | Haiku | **11,161** | 11,259 | 10,214 | 10,620 | 12 / 14 of 16 |

Brevi now uses fewer tokens than Python on all four: 7% fewer on the small tasks for both models, 3% (Sonnet) and 1% (Haiku) fewer on the big ones. The big-task lead is smaller than run-to-run noise, so call those two a tie for now. The big-task Brevi figures are from 2 fresh runs on the final v0.12 compiler (runs p3, p4); the small-task and Python figures are from runs p1 and p2. Python still passes first time more often on big tasks, and 5 of the 7 failed first tries left are empty lists or maps whose type the compiler could not work out, the next thing to fix.
Counted the old way, with every retry rewriting the whole file, big tasks stay behind Python (Sonnet 11,051 vs 9,141, Haiku 14,592 vs 12,688), so the patch-style retries matter as much as the compiler fixes.

What changed since v0.11: shorter error lines, typo fixes applied during the build, `brevi fix`, the Python spellings above, and fixes for every other first-try failure seen in runs p1 and p2 (8 of those 14 failed first attempts now pass as written). All 795 saved solutions that the v0.11 compiler builds print the same with v0.12.

Older results (v0.11, exact tokens, every retry counted as a whole file): small Sonnet 4,712 vs Python 4,453, small Haiku 7,341 vs 6,882, big Sonnet 12,461 vs 8,697, big Haiku 14,821 vs 10,613. Python's own totals moved between the two sessions (small Sonnet 4,453 then 5,016 on the same tasks), so compare languages within a session. Scored as patches, those same v0.11 runs give small Sonnet 4,712 vs 4,453, small Haiku 6,331 vs 6,882, big Sonnet 10,868 vs 8,697 and big Haiku 11,516 vs 10,184.

Tokens per finished task set (estimated; 18 tasks: code written plus errors read, average of 2 runs; Python from earlier runs of the same tasks):

| Model | Brevi v0.8 | Brevi v0.7 | Python | First try (Brevi v0.8) |
| --- | --- | --- | --- | --- |
| Sonnet | 4,309 | 4,363 | 4,021 | 36 of 36 |
| Haiku | 4,987 | 5,631 | 5,266 | 35 of 36 |

With v0.10 (Python truth, comprehensions, len/sum/sorted/any/all, optional return types), Sonnet's 18-task total is 4,074 tokens over 3 runs against Python's 4,021, with 54 of 54 tasks passing on the first try. Haiku was already below Python.

On the 8 bigger tasks (v0.9, 2 runs each), finished code is close to Python's size, but retries still cost more:

| Model | Brevi finished code | Python finished code | Brevi with retries | Python with retries | First try (Brevi) |
| --- | --- | --- | --- | --- | --- |
| Sonnet | 7,786 | 7,418 | 11,121 | 7,450 | 14 of 16 |
| Haiku | 9,705 | 9,751 | 17,107 | 10,634 | 9 of 16 |

Rust takes about 2.4 times as many tokens as Brevi on the hard tasks.

Speed (best of 3, seconds):

| Program | Python | Brevi | Rust |
| --- | --- | --- | --- |
| Word counts | 0.35 | 0.18 | 0.09 |
| Dijkstra | 2.90 | 0.47 | 0.30 |
| Prime sieve | 1.33 | 0.05 | 0.02 |
| Group records | 0.76 | 0.31 | 0.12 |

Not done yet:
- Full type inference: the check covers the common mistakes, and rarer type errors still come from rustc, translated
- Type inference for empty lists and maps that are only filled inside functions from values of unknown type
- Edit-by-name patches and compact output printing

## Build and try

```
cargo build --release
./target/release/brevi run examples/shapes.bv
./target/release/brevi test examples/shapes.bv
tests/check.sh            # regression tests: sample programs and error cases
```

Needs `rustc` on the PATH.

## Layout

| Path | What it is |
| --- | --- |
| `src/lexer.rs`, `src/parser.rs`, `src/ast.rs` | Source to syntax tree |
| `src/emit.rs` | Checks and Rust generation |
| `src/prelude.rs` | Runtime pasted into every generated program |
| `src/main.rs` | CLI, rustc invocation and error mapping |
| `examples/` | Sample programs |
| `tests/` | Expected outputs for programs and error cases |
| `tools/compat.py`, `compat/baseline.txt` | Every bench solution that ever passed; must keep passing on new toolchains |
| `tools/upstream_watch.py` | Opens review issues for new Rust, Python and crate releases |
| `.github/` | Daily toolchain matrix, upstream watch, auto-fix, Dependabot (see "Staying current") |

## Staying current

Brevi must never fall behind Rust or Python, and code that once passed must never silently break.

- **Toolchains** (`.github/workflows/toolchains.yml`, daily, on pushes to main, and Rust stable only on PRs): builds Brevi and reruns
  `tests/check.sh` plus `tools/compat.py check` (all 1,700+ saved Brevi, Python and Rust solutions that once passed)
  on Rust stable, beta and nightly, the newest Python and the next Python pre-release. Beta and nightly also report
  deprecation and future-incompatibility warnings in generated Rust and the newest edition, about 6 to 12 weeks before
  they reach stable. A toolchain already tested at the same commit is skipped. Failures open a `toolchain` issue.
- **Upstream releases** (`upstream.yml`, twice a week): one `upstream` issue per new Rust stable, Python release or
  newer crate pinned in tests/examples, listing the new language features and stabilized APIs with a review checklist.
- **Auto-fix** (`auto-fix.yml`): Claude Code picks up `toolchain` and `upstream` issues and opens a PR. Needs an
  `ANTHROPIC_API_KEY` repository secret; a human reviews every PR.
- **Dependabot**: Cargo dependencies and GitHub Actions versions, weekly.

After a change that makes more solutions pass, run `python3 tools/compat.py baseline` to lock them in.
Never remove a line from `compat/baseline.txt` to make a check pass.

## License

Apache License 2.0. See [LICENSE](LICENSE).

# Showcase binary size: breakdown and gate

Option A (one fast binary from a Python MCP server or CLI script) needs small binaries that still start fast.
Targets (Adam, 11-19): **time_mcp at most 8 MB, every showcase at most 10 MB, with no slower startup.**

## Tools

- `python3 bench/showcase/size.py BINARY [--top N] [--json]`: where the bytes go. ELF sections, code (.text) and
  data (.rodata, .data*) by crate, and the largest symbols. Linux only (readelf, nm). Crates: `brevi_prog` is the
  compiled program, `brevi_lib` the library copies it imports (src/dynpy/py: argparse, pydantic, the mcp SDK, ...),
  `brevi_dyn` the dynamic runtime, `brevi_rt` the typed runtime. A generic like `Vec<brevi_dyn::Obj>::drop` is charged
  to the Brevi crate it names. "unnamed" is section bytes no symbol covers (string literals, tables, padding).
- `python3 bench/showcase/run.py` writes a `size` breakdown, `binary_bytes` and `build_s` into every showcase in
  results.json.
- `python3 bench/showcase/run.py --gate [--targets] [--runs N]`: the gate. It builds and times the bundler showcases
  (sqlite_mcp, time_mcp, count_tokens_file, count_tokens_tree) and fails (exit 1) when, against baseline.json:
  - a binary is more than 5% bigger,
  - its first answer is more than 10% slower,
  - its release build takes more than 30% longer, or needs more than 30% more memory at its peak (so the size work,
    LTO and `codegen-units=1` above all, cannot quietly make builds slow),
  - its output differs from CPython's, or it does not build.
  A timed number over its limit is measured a second time and the better result counts; size is exact and is not.
  The build is timed on a second `brevi build --release --no-cache` after a first build has left the runtime rlib
  cached: rustc on the program and its library copies, as a rebuild after an edit runs. Peak memory is the largest
  resident size of the build or any process it waited for (rustc, the linker), from wait4.
  With `--targets` it also fails on a binary over its target. The gate writes no results.json.
- `python3 bench/showcase/run.py --record-baseline`: measures the gated showcases and writes baseline.json (and
  results.json). It refuses if any of them fails to build or differs from CPython.

Startup times only compare on one machine, so the gate refuses (exit 2) a baseline taken on another CPU instead of
skipping the check. The baseline and the gate run on Adam's PC (WSL), alone, not beside a CI run, with the mcp
stack, `mcp-server-time` and `tiktoken` installed as for results.json. A size lane (A2 release profile, A3 runtime
slicing, A4 embedded data) records a new baseline in its own PR once its gate passes, and the commit that does so
says by how much each binary shrank.

`--targets` is not on by default because no showcase meets its target yet (time_mcp was 19.2 MB on 11-10). Lane A2
turns it on in the PR that first meets the targets, so from then on the targets hold like the 5% rule.

## First breakdown (cloud container, main 20ab11d, the container's rustc 1.97.0, not the pinned 1.100.0; not the PC numbers)

Both are larger than results.json's 11-10 sizes (11.5 and 18.0 MB at 0.26.0): the runtime has grown since.

| | count_tokens | sqlite_mcp |
|---|---|---|
| file | 15.5 MB | 23.0 MB |
| .text | 7.7 MB | 14.7 MB |
| .rodata | 3.7 MB | 0.9 MB |
| read-only data no symbol names | 4.0 MB | 1.4 MB |
| symbol table (.symtab + .strtab) | 1.8 MB | 3.2 MB |
| .eh_frame + .gcc_except_table (unwinding) | 1.0 MB | 2.2 MB |
| .rela.dyn + .data.rel.ro (position-independent code) | 1.3 MB | 1.9 MB |
| code: brevi_dyn runtime | 5.6 MB | 7.1 MB |
| code: brevi_lib library copies | 1.1 MB (argparse, textwrap) | 6.6 MB (mcp models 0.9 MB, mcp 0.3 MB, pydantic, asyncio, sqlite3) |
| code: the program itself | 0.4 MB | 0.3 MB |
| code: std, core, alloc | 0.4 MB | 0.4 MB |
| release rebuild (`--no-cache`), peak memory | 21.4 s, 498 MB | |

What it points at, for the size lanes:
- A2 (release profile): `strip=symbols` removes the 1.8-3.2 MB symbol table outright. The linker already drops runtime functions
  nothing calls, so a token counter still carrying `np_module_call` and `make_dataclass` means the dynamic dispatch
  tables reach them. LTO and `codegen-units=1` shrink code by inlining and merging, but cannot drop what those tables
  reach; measure before promising a number.
- A3 (runtime slicing): the runtime code the dispatch tables keep alive, which only leaving modules out can cut.
- A4 (embedded data): count_tokens has 2.6 MB more unnamed read-only data than sqlite_mcp, most likely the tiktoken
  ranks and unicode tables. No symbol covers it, so A4 starts by naming it.
- In sqlite_mcp the library copies (6.6 MB) weigh as much as the runtime: the mcp models module alone is 0.9 MB of
  one init function.

The PC baseline (baseline.json, results.json) replaces this table's numbers once it is recorded.

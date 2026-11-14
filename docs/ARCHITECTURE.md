# How Brevi works

This document explains what Brevi does between a source file and a native program: which parts of `src/` run, in what order, and why. It describes `main` as of v0.27.1 (11 November 2026), plus what has merged since, marked "from the next release"; the file list below is checked against `src/` when it is revised.

Brevi reads two kinds of input:

- **Brevi files (`.bv`)**: Brevi's own language, written for AI models (guide in `BREVI.md`).
- **Python files (`.py`)**: plain Python, built natively where Brevi can match CPython's answer ("Python in, Rust speed out").

Both end the same way: Brevi writes Rust source, and `rustc` turns it into a native program. Python files have more steps, because a Python file must give exactly CPython's output or not be built natively at all.

## What runs natively

Every diagram below colours its steps the same way:

```mermaid
flowchart LR
    N["Brevi native<br/>compiled Rust, no CPython"]
    M["partly native<br/>native program, library calls in CPython"]
    P["not native<br/>runs on CPython"]
    N ~~~ M ~~~ P
    classDef native fill:#cdeccf,stroke:#2f8a3e,color:#10301a
    classDef mixed fill:#fdf0c2,stroke:#b58a12,color:#3a2a00
    classDef cpython fill:#f9d9c9,stroke:#c0522b,color:#3a1405
    class N native
    class M mixed
    class P cpython
```

| Path | Native? | What runs |
| --- | --- | --- |
| Typed build (every `.bv`, most `.py`) | **Brevi native** | Plain Rust with fixed types. The fastest path. |
| Dynamic build (`src/dynpy`) | **Brevi native** | Rust over a dynamic value type. No CPython anywhere. Slower than typed, usually faster than CPython. |
| Library calls (pybridge) | **Partly native** | The program is native; each call into numpy, pandas, etc. runs in CPython loaded into the program. |
| CPython fallback (`brevi run` only) | **Not native** | The whole file runs on `python3`, with a one-line path report on stderr. |

The release test counts how many final programs build natively (typed or dynamic, no fallback); the bar is 90%.

### Which path ran

`brevi run`, `brevi build` and `brevi check` say which of these paths a Python file took, as one JSON line on stderr, so an agent can tell when it is off the fast path and which function to rewrite:

```
{"path":"dynamic","line":12,"fn":"parse","why":"rows holds [int] (set on line 9) but is set to [str] here"}
{"path":"pybridge","why":"numpy run in CPython","py":["numpy"]}
{"path":"cpython","line":2,"fn":"g","why":"Brevi could not build it: unknown function eval()","by":"python3"}
```

| `path` | Meaning |
| --- | --- |
| `typed` | Typed build: the fast path. Not printed unless `--explain` or `BREVI_PATH=1`. |
| `pybridge` | Typed build; `py` lists the modules (and functions of Brevi's own modules it lacks, like `math.gcd`) that run in CPython. |
| `dynamic` | Dynamic build. `why` is the typed build's first refusal (or the guard's reason), `line` and `fn` where it is, `file` when it is in an imported module. `py`, when present, lists the libraries (and `open`) that CPython runs inside the program (see [Library calls](#library-calls-pybridge)). |
| `cpython` | `brevi run` handed the file to `python3` (`by`). `why` is Brevi's first build error. |

With `--explain` the line also carries the error's `fix`, up to five more typed errors (`also`), and for `cpython` why the dynamic build refused the file (`dynamic`). `BREVI_PATH=1` (or `--strict`) prints the line for every path, `BREVI_PATH=0` never. A failed build prints only its errors. The MCP server always sets `BREVI_PATH=1` and returns the report as `structuredContent` (see `mcp.rs`). The code is `say_path` in `main.rs`.

## The big picture

```mermaid
flowchart TD
    SRC["source file<br/>.bv or .py"] --> FRONT["front end<br/>modules, caps, ffi"]
    FRONT --> GUARD{".py only:<br/>typed_guard<br/>would typed differ<br/>from CPython?"}
    GUARD -- "no (and every .bv)" --> TYPED["typed build<br/>parser, inference, checks, emit"]
    GUARD -- "yes" --> DYN["dynamic build<br/>src/dynpy"]
    TYPED -- "Rust" --> RUSTC["rustc, or cargo if crates<br/>+ runtime library"]
    TYPED -- "front-end error<br/>or rustc error, .py only" --> DYN
    DYN -- "Rust" --> RUSTC2["rustc<br/>+ dynamic runtime library"]
    RUSTC --> BIN["native program"]
    RUSTC2 --> BIN
    DYN -- "cannot build" --> FAIL{"command?"}
    TYPED -- "error, .bv" --> ERR["JSON errors<br/>with fix hints"]
    FAIL -- "brevi run" --> PY["run with python3<br/>+ one-line path report"]
    FAIL -- "build, check, --native" --> ERR
    classDef native fill:#cdeccf,stroke:#2f8a3e,color:#10301a
    classDef mixed fill:#fdf0c2,stroke:#b58a12,color:#3a2a00
    classDef cpython fill:#f9d9c9,stroke:#c0522b,color:#3a1405
    class TYPED,DYN,RUSTC,RUSTC2,BIN native
    class PY cpython
```

In words, for a Python file:

1. **Guard.** `typed_guard.rs` reads the file (and the local modules it imports) and looks for code the typed build would answer differently from CPython, such as printing a set. If it finds any, the typed build is skipped.
2. **Typed build (Brevi native).** The normal path: infer types, check, and emit plain Rust with fixed types. This is where the big speedups come from. Brevi's own spellings are mistakes in a `.py` file, so `pydiag.rs` stops on them before and after this build: Brevi syntax (`&&`, `x => x + 1`, `fn f(x):`), Brevi names (`true`, `fail("x")`), and a method only Brevi has, called on a value the typed build knows is a list, dict, text or number (`xs.push(1)`; a class's own `push` is fine).
3. **Dynamic build (Brevi native).** If the typed build fails at any stage (its own checks or `rustc`), `pydiag.rs` first looks at the typed errors for sure mistakes: a name that is never defined, a method no Python type has, or text added to a number. Code that may never run (under `sys.version_info`, `sys.platform`, `TYPE_CHECKING` or `if False`, and `except` blocks) is skipped. `check` and `build` report these as errors instead of building; `brevi run` falls back to python3 and names the mistake in its path report. Otherwise the file is rebuilt over a dynamic value type that behaves like CPython's objects. Still native, no CPython involved, with one `{"path":"dynamic"}` line saying why (see [Which path ran](#which-path-ran)).
   **An int past 64 bits.** The typed build's ints are 64 bits and Python's have no limit, so a value that outgrows them is never wrapped or capped: the run stops at once (`ax_i64_stop` in the prelude). Under `brevi run` the program then runs again from the start on the dynamic build (`src/replay.rs`), with a `{"path":"dynamic","why":"the typed build stopped (an int passed 64 bits), ..."}` line: it is given the same stdin, it must write what the first run wrote before anything new, and only the new output is passed on. The first run writes straight to stdout and stderr (the prelude counts and hashes what it wrote; the second run's output is checked against that) and reads stdin itself (what it took from a pipe is handed over; a file is read again from where it started). The same happens for a result equal to one of the typed build's marks for None (i64::MIN, f64::MIN, Decimal's i128::MIN, the text "\0None"): `ax_ck` checks each + - * and unary minus that leaves an arithmetic expression. Only programs the build proves touch nothing but stdin, stdout, stderr and files they read are run twice (caps.rs: no file writes, network or native code); others, `--limit` runs and binaries run by themselves stop with an error that says why. `(a * b + c) % m` on ints is worked out in 128 bits, so such hashes never need this.
4. **CPython fallback (not native).** Only if the dynamic build refuses too. `brevi run x.py` then runs the file with `python3` and prints one `{"path":"cpython"}` line on stderr. `brevi build`, `brevi check` and `--native` (or `BREVI_NATIVE=1`) never fall back; they print the errors.
5. **`--strict` (or `BREVI_STRICT=1`, from the next release): no fallback at all.** Only step 2 may answer (pybridge included). `dyn_fallback` in `main.rs` is the one place every typed-to-dynamic fallback goes through (typed errors, rustc errors, the guard, the soft guard); under strict it prints one `{"e":"strict","path":"dynamic",...}` error and exits 1 when the dynamic build would have taken the file. Strict implies `--native`, turns off the int replay (`replay.rs`), makes `brevi-python` refuse every CPython handoff (`shim.rs`), and prints the path line for every build. `--strict` sets `BREVI_STRICT=1` for every child process, and the result cache keys on every `BREVI_*` variable, so a cached non-strict build is never replayed under strict. Tests: `tests/strict/`, `#== strict split` in `tests/check.sh`.

A Brevi file only takes the typed path. Its errors go straight back to the model as JSON.

## Commands

All commands live in `src/main.rs`.

| Command | What it does |
| --- | --- |
| `brevi run FILE` | Build, then run. For `.py`, falls back to `python3` if no native build works. |
| `brevi build FILE [-o OUT]` | Build a native program. |
| `brevi check FILE [--fast]` | Front-end checks, then `rustc --emit=metadata` (no binary). `--fast` skips `rustc`. Replayed from the result cache when the files did not change. |
| `brevi test FILE` | Build with `--test` and run the program's `test` blocks. |
| `brevi emit FILE` | Print the generated Rust. |
| `brevi fix FILE` | Apply every suggested fix to the file. |
| `brevi patch FILE PATCH` | Replace whole functions/types by name (`src/patch.rs`). |
| `brevi guide [SECTION]` | Print the language guide or one section. |
| `brevi verify` | Native build against CPython on the same inputs, with a certificate (`src/verify.rs`). |
| `brevi run --verify FILE` | One native run checked against CPython on the same stdin and arguments, CPython running alongside (`verify.rs`, [VERIFY_BY_DEFAULT.md](VERIFY_BY_DEFAULT.md)). A different answer gives CPython's output and is logged; a program a second run would change (files, network, clock, environment) runs unverified. |
| `brevi report [--clear]` | The wrong answers `run --verify` logged, each with an issue body to paste. Local only. |
| `brevi mcp` | MCP server over stdin/stdout (`src/mcp.rs`): guide, check, run, test, verify, patch, edit, show. `brevi mcp --http [HOST:]PORT` serves the same tools over Streamable HTTP (`src/mcp_http.rs`), with a bearer token off 127.0.0.1 and an Origin check. From v0.27.0 the server keeps every program sandboxed and refuses a call that turns the sandbox off or grants anything, unless `--allow` / `--allow-py` / `--allow-net` / `--allow-read` / `--allow-write` set what calls may ask for or `--trust-calls` trusts them fully. |

Flags: `--release` (opt-level 3), `--sandbox` (only what the caller grants with `--allow read,write,net,native` / `--allow-net HOSTS` / `--allow-read DIRS` / `--allow-write DIRS` / `--allow-py MODULES`; the file's allow lines grant nothing), `--limit cpu=S,mem=MB,time=S,output=KB,file=MB,written=MB,net=MB,threads=N,fds=N,files=N` (resource limits on the program run: kernel rlimits set before it starts, wall-clock time and output held by `proc::run`, threads by a pids cgroup per program (`cgroup.rs`), files, bytes written to files and network bytes counted in the program by `fsscope.rs`; see [THREAT_MODEL.md](THREAT_MODEL.md)), `--window` (real tkinter windows via egui), `--explain`, `--native`.

## Stage 1: front end

These steps run on the source text before parsing.

| Step | File | What it does |
| --- | --- | --- |
| Join files | `modules.rs` | `use "x.bv"`, and `import x` of a sibling `.py` file or package, are joined into one program. A segment table maps every line back to its file, so errors name the right file. |
| Capabilities | `caps.rs` | Under `--sandbox`, only what the caller grants (`--allow read,write,net,native`, `--allow-net HOSTS`, `--allow-py MODULES`, the MCP `allow` / `allow_net` / `allow_py` arguments) counts, and an allow line in the file that asks for more is refused. Without it, a program with `allow read, write` lines is held to them. A program that uses something not granted is refused, before any other path runs: a refused program is never built over dynamic values, through pybridge or run with python3. Python libraries (pybridge) need `allow native`, and a sandboxed program never falls back to python3. In a `.py` file the line may be a comment (`# allow read`). Path text (`os.path` functions, the `os` path constants and `pathlib.PurePath`) is built from Brevi's own Python copies (`src/dynpy/py`, over dynamic values, no file access) with or without the sandbox, so a program that only handles path text stays native; without `allow native`, `random` uses Brevi's copy too, and `caps::path_uses` refuses every other use of os and pathlib by name. Brevi's `requests` and `httpx` (`src/dynpy/py`) need `net`: their sockets and TLS are in `dynpy/netglue.rs`, pasted into a program only when it imports `_brevi_net`, and every connection passes the host check `--allow-net` sets (the runtime every program carries reaches no network). |
| Foreign code | `ffi.rs` | `use CRATE "VER"`, `rust fn`, `py fn`, `rust:` and `py:` blocks. Foreign bodies are cut out and replaced by a stub on the same line, so line numbers stay put. Any crate switches the build to cargo. |
| Autofix | `main.rs` `autofix` | For `.bv` only: errors with a sure one-word fix (a typo of a known name) are fixed before the build and reported on stderr. Python files are never rewritten, because CPython would not rename `s.rpartition` either. |

A file counts as Python if it ends in `.py`, or if it uses spellings only Python code has (`def`, `class`, `import`, `None`, `.append(`, ...). See `python_style` in `main.rs`. Python mode turns on the Python-only passes below.

## Stage 2: the typed build

> **Brevi native.** Everything this stage produces is compiled Rust. Only values from bridged libraries (below) are handed to CPython.

```mermaid
flowchart LR
    A["lexer.rs<br/>tokens + indentation"] --> B["parser.rs<br/>AST (ast.rs)"]
    B --> C["gui.rs / pybridge.rs<br/>hooks during parsing"]
    C --> D["bigint.rs<br/>big-int promotion"]
    D --> E["alias.rs<br/>Python sharing"]
    E --> F["records.rs<br/>dicts as records"]
    F --> G["infer.rs<br/>types + checks"]
    G --> H["emit.rs auto_try<br/>error passing"]
    H --> I["moves.rs<br/>ownership"]
    I --> J["emit.rs<br/>Rust source"]
    classDef native fill:#cdeccf,stroke:#2f8a3e,color:#10301a
    classDef mixed fill:#fdf0c2,stroke:#b58a12,color:#3a2a00
    classDef cpython fill:#f9d9c9,stroke:#c0522b,color:#3a1405
    class A,B,D,E,F,G,H,I,J native
    class C mixed
```

The driver is `compile_once` in `main.rs`. In order:

1. **Lex and parse** (`lexer.rs`, `parser.rs`, `ast.rs`). One parser reads both Brevi and Python spellings into one syntax tree. Python forms are rewritten into Brevi ones as they are parsed: nested `def`s are lifted, `len`/`sum`/`append` become Brevi methods, classes become types with methods. A syntax error is a JSON line with the line number and, where it can, a fix.
2. **Parser hooks.** `gui.rs` takes calls on tkinter widgets. `pybridge.rs` marks every name that holds a value from a Python library (see [Library calls](#library-calls-pybridge)). Because a name can be used before the line that assigns it, the parser runs again while new names are found.
3. **Big integers** (`bigint.rs`, Python mode). Python ints have no size limit. A number read from text that is only added, multiplied, compared and printed becomes a big integer. Powers of literals are worked out at compile time, so `3 ** 50 % 1000000007` gives Python's answer.
4. **Sharing** (`alias.rs`, Python mode). In Python, `r = rows[k]; r[0] -= 1` changes `rows`. Brevi values are copies, so after each statement that changes `r` in place, the copy is written back (`rows[k] = r`). Where a change would still be lost, the build is refused instead of giving the wrong output.
5. **Records** (`records.rs`, Python mode). A dict with fixed string keys and values of different types (`{"n": n, "date": d, "posts": []}`) becomes a record type, like a class. It still prints as Python prints the dict and still serialises with `json.dumps`.
6. **Inference and checks** (`infer.rs`). Works out the types Brevi programs leave unstated: the item type of `xs = []` from later uses, return types, which parameters a function changes (`auto_mut`), and which method a call on an object means. Then `infer::check` runs Brevi's own type checker, with short errors such as "str + int" or "type changes from int to str", each with a fix.
7. **Error passing** (`emit.rs` `auto_try`). A call to a function that can fail passes its error up unless the code already handles it, so programs need no `?`.
8. **Ownership** (`moves.rs`). A variable's last read moves the value instead of copying it. If moving breaks Rust's borrow rules, the build retries without moves (see Stage 4).
9. **Emit** (`emit.rs`). Writes one Rust file: the runtime prelude, then the program. Each statement records its source line (`ax_l(n)`), so a runtime error can name the Brevi or Python line.

The typed build may also ask `pybridge::learn` to bridge a missing library function (for example `math.gcd`) and build again, up to 8 rounds (`compile_to_rust`).

### What "Python behaviour" means in the typed build

The typed build copies the CPython behaviour that shows in a program's output: integer size (`bigint.rs`), shared objects (`alias.rs`), dict printing and JSON (`records.rs`), set membership with a hash index (`emit::FAST_SETS`), Python's float and `round` formatting, exception messages (`ax_exc_msg` in `prelude.rs`), and so on. Anything it cannot match is not built typed. Either the typed build refuses it (and the dynamic build gets a turn), or `typed_guard.rs` sends it to the dynamic build before the typed build starts.

## The guard: when typed would be wrong

Some Python programs type cleanly but would still print something different from CPython. `typed_guard.rs` finds them on the Python syntax tree (it reuses the dynamic build's parser) and sends them straight to the dynamic build. Current cases:

- **Set order.** The typed build keeps a set sorted. CPython walks a set in hash-table order (`{3, -1, 5}` prints `{3, 5, -1}`). Printing, iterating, `list(s)`, `s.pop()` or joining a set goes dynamic. Order-free uses (`x in s`, `len`, `sorted`, `min`, `sum`, set algebra, and a comprehension over a set handed straight to `max`, `sorted`, `any`, `len` and the like) stay typed. A set of strings (or of tuples led by one) has no fixed order in CPython, whose string hashes change every run, so walking or printing it stays typed too: `check_soft` names the sets, `emit` adds an `ax_rand_set(&name)` mark after each place one is set, and rustc only accepts it when the elements are strings (`AxRandElem` in `prelude.rs`). If any mark fails, the file goes where the guard would have sent it.
- **`tuple(xs)` printed.** The typed build holds such a tuple as a list, so `print(tuple(xs))` would show `[1, 2]`. The guard follows such values into lists, through a helper's parameter (`out.append(tuple(cur))`) and through returns; `m.groups()` counts as one too.
- **Argument order.** `print(q, q.popleft())`, `print(xs, grow(xs))`, `r = (q, q.pop())`: CPython reads every argument first and shows the object itself; the typed build copies each argument as it goes, so `q` would show the value from before the change. For calls, the guard works out what each function of the file changes in place (`program_changes`: its parameters, globals and closure names, through the functions it calls), and sends the file off when a later print argument, or tuple or list item, changes what an earlier one holds. Functions in other modules are not followed.
- **Regex matches** printed directly (`<re.Match object ...>` versus a list of groups). (A group that did not match is the None string in a typed match, so `m.group(2) is None` and printing it stay typed.)
- **Changing a dict or set while looping over it**, which CPython stops with an error.
- **Shared containers** (`share_guard.rs`, from v0.27.0). Python names objects, and the typed build copies lists, dicts and sets. The two agree exactly when no container is changed through one name while another name that sees the same object is still read. This pass proves that on the Python tree, function by function with summaries across calls, and sends every file it cannot prove to the dynamic build. Where the proof relies on the typed build sharing (a `mut` parameter, a loop that changes items in place, a class instance as a handle), it records the need, and `main.rs`, `emit.rs` and `alias.rs` check the typed build met it; a file where it did not goes dynamic too.
- **Annotations that could raise** (`ann_guard.rs`, from the next release). CPython 3.13 evaluates module, class and def annotations when the statement runs, so `def f(a: Later)` above `class Later` raises `NameError`. This pass finds annotations that could raise (an unbound name, a bad subscript checked against `src/pynames/ann_types.txt`, made by `tools/gen_ann_types.py`) and sends the file to the dynamic build, which evaluates them as CPython does.
- **Generators** that cannot safely be run early. The typed build turns a generator into a function that returns a list, which is only correct when running it early cannot be told apart.

A file the guard cannot read (its parser rejects it) has passed none of these checks, so the typed build may not build it either: if the typed front end accepts it, it is refused with the guard's reason. A file the typed build refuses too keeps the typed errors. Either way `brevi run` falls back to CPython.

`BREVI_GUARD=print brevi build x.py` prints the guard's verdict.

Before the guard and the typed build, `pysentinel.rs` rewrites a sentinel `MISSING = object()` to None when the program cannot tell the two apart (see the rules at the top of that file).

## Stage 3: the dynamic build (`src/dynpy`)

> **Brevi native.** No CPython is involved: the program and its runtime are compiled Rust.

For Python files the typed build cannot handle, such as a variable that holds an int and later a string, or a list of mixed values.

```mermaid
flowchart LR
    P["dynpy/parse.rs<br/>own Python parser"] --> M["imports of sibling .py<br/>files collected"]
    M --> E["dynpy/emit.rs<br/>Rust over enum V"]
    E --> R["rustc, opt-level 1<br/>(3 with --release)"]
    L["rt_*.rs runtime<br/>prebuilt rlib, opt-level 3"] --> R
    R --> B["native program"]
    classDef native fill:#cdeccf,stroke:#2f8a3e,color:#10301a
    classDef mixed fill:#fdf0c2,stroke:#b58a12,color:#3a2a00
    classDef cpython fill:#f9d9c9,stroke:#c0522b,color:#3a1405
    class P,M,E,R,L,B native
```

- **Its own parser** (`dynpy/parse.rs`) for a supported subset of Python. Anything outside the subset is an error at build time with a line number.
- **The whole program goes dynamic**, not single variables. Every value is a `V` enum (int, float, str, list, dict, object, ...) with CPython's semantics: CPython's hashing and set order, its formatting, its exceptions, classes and method lookup.
- **Runtime** (`rt_core` values, `rt_ops` operators, `rt_fmt` formatting, `rt_call` calls and classes, `rt_lib` builtins, `rt_mod` stdlib modules, `rt_text` regex engine and json, `rt_ext` decimal/fractions/datetime/csv/statistics/html/threading, `rt_enum` enum, `rt_glue` helpers the emitted code calls, `rt_gen` generators, `rt_async` coroutines and asyncio's event loop, `rt_bytes` bytes/hashlib/base64, `rt_typing` typing and collections.abc, `rt_zone` zoneinfo (the zone database in `zoneglue.rs`), `rt_rand` the Mersenne Twister behind Brevi's `random`, `rt_gc` `gc.collect()`'s cycle collector and `weakref`, `rt_col` the column kernels behind Brevi's polars, `rt_bytearray` and `rt_memview` bytearray and memoryview (from the next release), `rt_bdoc` the builtins' docstrings, plus the small-object allocator `alloc_rt.rs`). The native library engines live next to them: numpy in `rt_np*` (`rt_np` arrays, `rt_npfn` functions, `rt_npfmt` printing, `rt_nplinalg`, `rt_nplsq`, `rt_nprand` and `rt_nprdist` bit-exact random, `rt_npsort` numpy's exact argsort order), tiktoken in `rt_tok*` (`rt_tokre` the regex engine for its split patterns, with `tok.rs` fetching and checking the vocabularies at build time), Hugging Face tokenizers in `rt_hf*` (`rt_hfcore` the pipeline, `rt_hfnorm` strings with alignments, `hf.rs` reading tokenizer.json at build time), SQLite in `rt_db`, safetensors in `rt_st`, ONNX Runtime in `rt_ort`, and sockets, TLS and HTTP framing in `rt_net` (whose system sides, `procglue.rs` for child processes, `stdioglue.rs` and `srvglue.rs` for MCP over stdio and HTTP, are pasted into a program only when it uses them). From the next release the dynamic runtime also has `rt_complex` (complex numbers), `rt_bigrange` (`range()` past 64 bits), `rt_thread` (threads on their own stacks, with `Lock`, `Event`, `Condition`, `Semaphore` and `Barrier`), `rt_itred` and `rt_itred_comb` (CPython's `__reduce__` and `__setstate__` state for iterators, so they pickle and copy), `rt_sig` (`inspect.Signature.from_callable`), `rt_deque` and `rt_odict` (`collections.deque` and `OrderedDict` with CPython's mutation, comparison and pickling rules) `rt_tdict` (exact class `__dict__`, `vars()` and `dir()`) and `rt_trace` (`sys.settrace` with CPython 3.13's events, for the programs `trace_check.rs` accepts; `dis` is refused, a compiled function having no bytecode); `mangle.rs` mangles private names as CPython's compiler does. `rt_tdict_data.rs` and `cpy_lines.rs` are generated from CPython 3.13.13 by `tools/gen_type_dicts.py`. Files named `*uni.rs` (`rt_hfuni.rs` among them), `rt_hfcls.rs` and `rt_tokuni.rs` are tables generated by `tools/gen_*.py`; `src/unicode.rs` (CPython 3.13's tables for `casefold()` and the `is*` digit tests) is compiled into this runtime and pasted into typed programs that use them. It is built once per rustc version into `~/.cache/brevi/dynrt/<hash>` and linked into each program.
- **Modules.** 36 standard modules are built in, plus the `_brevi_*` modules behind the native engines (`BUILTIN_MODULES` in `dynpy/emit.rs`: math, re, json, collections, itertools, enum, decimal, datetime, csv, zoneinfo, ...; `gc` and `weakref` only for the names `module_has` lists). A few more are Brevi's own Python copies in `src/dynpy/py`, compiled with the program like its own files: path text from `os`, `os.path` and `pathlib`, `io.StringIO`, `random` under the sandbox, `asyncio`, `argparse`, `logging`, `contextlib`, `textwrap`, `tiktoken`, `tokenizers`, `requests` and `httpx` over `_brevi_http` and the native `_brevi_net`, and the library copies (pydantic, the `anthropic`, `openai` and `mcp` SDKs, jinja2, polars, sqlite3, duckdb, safetensors, onnxruntime, ...; `dynpy::sandbox_module`). Brevi's pydantic needs help from the compiler, because pydantic reads a model's annotations as typing objects: `dynpy/pyd.rs` checks the models and rewrites them before the build. Those that touch files or native engines do it through glue pasted into the program only when used (`fsglue.rs` for files, `sqlglue.rs`, `duckglue.rs` and `ortglue.rs` for the SQLite, DuckDB and ONNX Runtime libraries, `uname_glue.rs` for re's `\N{NAME}` escapes; `libpin.rs` pins each engine library to the SHA-256 it had at build time, from the next release), which checks the read and write grants, so the runtime itself never opens a file or library. Any other import (pandas, yaml, ...) is bridged: CPython runs that library inside the program, and the rest stays native (see [Library calls](#library-calls-pybridge)). Without `allow native` under the sandbox, or without a python3 that has libpython, those imports are refused at build time.
- **Refuse at build time, never at run time.** Anything unsupported (`module_has`, `is_missing_builtin`) must stop the build, so `brevi run` can still fall back to CPython. A program that builds must give CPython's output.

Typed builds are faster. The dynamic build was about 1.3x CPython on micro benchmarks and 4.5x on the `minilang` tests when it landed; on the 12 compute benchmarks (12 Nov 2026, Ryzen 9 5950X) it runs at 0.5x to 1.4x CPython's speed, median 0.85x, where the typed default build is 1.6x to 37.9x faster, median 9.3x.

Switches: `BREVI_DYN=1` forces the dynamic build, `BREVI_DYN=0` disables it, `BREVI_DYN_DEBUG=1` prints `rustc` errors from generated code.

## Library calls (pybridge)

> **Partly native.** The program is native; only the library calls themselves run in CPython.

For Python files that import libraries Brevi does not build itself (pandas, yaml, ...).

```mermaid
flowchart LR
    S["pybridge.rs scan<br/>which imports are bridged"] --> T["parser marks values<br/>from bridged modules as Py"]
    T --> O["each operation becomes<br/>ax_pyb_call / ax_pyb_op"]
    O --> RT["pyrt.rs pasted into<br/>the program"]
    RT --> CP["at run time: dlopen libpython,<br/>CPython runs the library call"]
    classDef native fill:#cdeccf,stroke:#2f8a3e,color:#10301a
    classDef mixed fill:#fdf0c2,stroke:#b58a12,color:#3a2a00
    classDef cpython fill:#f9d9c9,stroke:#c0522b,color:#3a1405
    class S,T,O,RT native
    class CP cpython
```

- `pybridge::scan` reads the import lines. A module Brevi has built in (math, re, json, collections, ...) stays native. Any other module is bridged.
- The parser asks `pybridge.rs` about every name: a bridged module, a name imported from one, and every value made from those (`a = np.array(xs)`, `a.sum()`, `a * 2`) has type `Py`.
- Each operation on a `Py` value becomes a call into the runtime (`ax_pyb_call(o, "sum", ...)`, `ax_pyb_op("+", a, b)`). The runtime (`pyrt.rs`, pasted into the program only when needed) loads `libpython` the first time a library is used, so the result is CPython's own.
- The rest of the program stays native. Brevi values are converted on the way in; Python values come back as Brevi values only through `int()`, `float()`, `str()`, `bool()` and `len()`.
- The program needs `libpython` on the machine where it runs, unless it is built with `--standalone`. That build appends CPython, the shared libraries it needs (glibc aside), the standard library and the imported packages to the binary (`src/pybundle.py`), and `pyrt.rs` unpacks them once into `~/.cache/brevi/py/<hash>` and loads libpython from there. Standalone builds are Linux only, and nothing here runs on Windows.
- A library program the typed build rejects goes to the dynamic build, which bridges the same libraries its own way (`src/dynpy/pyglue.rs`, pasted after `src/pyrt_load.rs`, the loader both bridges share). A library value is an `Ext` value holding a Python object, and every operation on it runs in CPython. None, bools, numbers, strings, bytes and tuples cross as the program's own values. Lists, dicts and sets returned by a call are copied in, and those reached through an attribute or item stay the library's own objects, so `post.tags.append(x)` changes what the library sees. A list or dict the program passes in and the library changes is copied back after the call. Functions of the program go in as Python callables; a def, lambda or bound method goes in as a stand-in (`_BrFn`, `_BrMeth` in pyglue.rs) that a library sees as CPython's function or method (names, doc, defaults, attributes, co_flags, `inspect.isfunction`, binding in a class), and whatever compiled code has no true answer for (bytecode and the other `co_*` fields, `__globals__`, `__closure__`) raises the dynamic build's `NotImplementedError` refusal, so `dis` or `inspect.signature` reached through `importlib` never answers wrongly. Exceptions keep their class names, bases and attributes both ways. Without bridging, `open()` is refused by the dynamic build. With bridging, `open()` is CPython's.
- A class built on a library class (`class Post(BaseModel)`, or a class with a library decorator) is run by CPython whole. Its lines are compiled in a Python module named like the program's, after that file's import lines (`dynpy::lib_classes`). Such a class is refused, so the file runs in CPython, in three cases: it sits inside a function or block, it uses a name of the program that CPython cannot see, or a class in another file builds on it.
- The `{"path":"dynamic"}` line names what CPython runs (`"py":[...]`). The dynamic runtime itself still reaches no file, process or CPython. The bridge code is added to the program only when the program imports a library.

### Plain Python in a `.bv` file

`bv_as_py` in `src/main.rs` decides, before anything else runs, whether a `.bv` file is plain Python: Python-style (`python_style`), compilable by CPython (`pydiag::not_python`), free of Brevi-only names (`pydiag::scan_brevi`), with no plain string holding `{name` (Brevi interpolates it, Python does not), and with no function ending in a bare value (`bvpy::returns_tail`: Brevi returns it, Python drops it). Such a file is marked in `BV_PY`, and `is_py(file)` (used wherever `main.rs` once tested `.py`) treats it as Python: the guard, the `.py` rewrites, the typed build with Python's meaning and the dynamic fallback. `--native` is implied, so it never runs in python3. `BREVI_BV_PY=0` turns this off.

A Python-style `.bv` file that fails those tests only by Brevi's spellings is rewritten to Python by `bvpy::to_python` (line for line: `fn` to `def`, implicit returns, `=>` lambdas, interpolating strings to f-strings, `name: value` arguments, `&&`/`||`/`!`, `true`/`false`, `.push`, `a..b` in `for`), and when the rewrite passes the same tests `dynpy::set_source` hands it to the dynamic build, which then reads it instead of the file. The typed build still goes first on the file as written; only its refusal reaches the dynamic build. `BREVI_BV_DYN=0` turns this off; `print` shows the rewrite.

### Libraries on Brevi's engine (`.bv` files)

> **Brevi native.** A `.bv` file whose libraries Brevi all has copies of runs them without CPython.

A Brevi file only takes the typed path, so its library calls go through pybridge too. When every library it imports is one the dynamic build has natively (numpy, tiktoken, tokenizers, requests, httpx, anthropic, openai, pydantic, jinja2, markupsafe, polars, sqlite3, duckdb, safetensors, onnxruntime, textwrap, io, argparse, and Brevi's built-in modules), `engine.rs` puts those calls on the dynamic build's runtime instead:

- `engine_rt.rs` is a second implementation of `pyrt.rs`'s calls (`AxPy`, `ax_pyb_call`, `ax_pyb_op`, conversions, write-back, lambdas as callables), over `brevi_dyn::V`. The typed build's lowering is the same either way.
- The libraries are compiled by the dynamic build from a stub: one `import` per module the program uses, and every library call the typed program makes (`engine::shapes`, read off the `ax_pyb_*` calls after parsing) repeated with the same names, keyword names and string literals in a function that never runs. So the dynamic build's own build-time refusals (a numpy method its copy lacks, an unknown tiktoken encoding, a polars feature) still apply; a refused stub keeps CPython's libraries. `BREVI_ENGINE_DEBUG=1` prints the refusal and the stub.
- The dynamic build's Rust goes into a `mod __dyn` of the typed program (its `main` becomes `__bv_main`, run before the first library call), and rustc links the dynamic runtime library (`--extern brevi_dyn`) next to the pasted prelude. That runtime brings its allocator, so the prelude's copy is left out.
- Off for `.py` files (their dynamic build is tried first), under `--sandbox`, on Windows, and with `BREVI_ENGINE=0`. The path line says `typed`.

## Stage 4: compiling the Rust

> **Brevi native.** Typed and dynamic builds both end here, as a native executable.

```mermaid
flowchart TD
    V["up to 4 versions of the Rust<br/>(hash-indexed sets on/off x inferred types on/off)<br/>each with and without moves"] --> K{"build cache hit?<br/>~/.cache/brevi/bin"}
    K -- "yes" --> OUT["copy or hard-link binary"]
    K -- "no" --> C{"crates?"}
    C -- "no" --> RC["rustc, linked against<br/>prelude rlib (dev)<br/>or pasted prelude (--release)"]
    C -- "yes" --> CG["cargo build<br/>shared target dir"]
    RC -- "ok" --> OUT
    CG -- "ok" --> OUT
    RC -- "borrow error" --> PLAIN["retry same version<br/>without moves"]
    RC -- "other error" --> NEXT["try next version"]
    NEXT --> REP["no version builds: map rustc<br/>errors back to source lines (JSON)"]
    classDef native fill:#cdeccf,stroke:#2f8a3e,color:#10301a
    classDef mixed fill:#fdf0c2,stroke:#b58a12,color:#3a2a00
    classDef cpython fill:#f9d9c9,stroke:#c0522b,color:#3a1405
    class RC,CG,OUT native
```

- **Versions.** The typed build makes up to four Rust versions of the program: with and without inferred types for empty values, and with and without hash indexes for Python's searched lists. Each also has a variant without moves. `rustc` gets them in order; the first that compiles wins. Errors are reported from the first version, since its inferred types name the real gap. Only the first version is made up front: each other one is a whole front-end pass, made only when the versions before it failed (making all eight first cost a cached check up to 5 s).
- **Runtime library.** The prelude (`prelude.rs` plus `alloc_rt.rs`, about 6,000 lines) is built once per rustc version into `~/.cache/brevi/rt/` as an rlib at opt-level 3. Normal builds link it and compile only the program at opt-level 1, which skips type-checking the prelude on every build. `--release` pastes the prelude into the program instead, so its functions can be inlined, and compiles at opt-level 3.
- **Crates.** A program with `use CRATE "VER"` builds through cargo, with one shared target dir (`$BREVI_CACHE` or `~/.cache/brevi/target`).
- **Result cache** (`runcache.rs`). `check`, `build` and `run` (and `run`'s inner build for a Python file) run the build as a child `brevi check` or `brevi build` with `BREVI_PHASE=1`, and keep what it printed, its exit code and its binary in `~/.cache/brevi/runs/`, under a hash of the program's files (every `.py` and `.bv` file in its folder and the folders its imports and `use` lines reach), the command and flags, the `BREVI_*` and Python settings, the Brevi binary (version, size, time) and the toolchain. An unchanged program then replays in about 10 ms with no front end and no `rustc`, path report line included; the MCP server's calls get this too. A refusal is kept only when the program imports nothing outside its folder and the standard library, and never one without a line in the program (an error about the machine).
- **Build cache.** Below it, builds are cached by a hash of the generated code, the flags, the rustc version and the prelude, so a program whose generated Rust did not change skips `rustc`. Failed builds are cached too, except errors that point at no code (a folder that could not be written). Both caches together keep at most `BREVI_CACHE_MB` megabytes (default 1024), least recently used first out. `--no-cache` or `BREVI_NO_CACHE=1` turns both off.
- **rustc itself.** Brevi asks the rustup proxy once for the toolchain's sysroot and then calls that rustc directly (each proxy call costs 20 to 50 ms). The answer is kept in `~/.cache/brevi/toolchain/` under a key of PATH, the RUSTUP_* settings, rustup's settings file and any rust-toolchain file above the working directory. Builds without `--release` use one codegen unit per core (4 to 16) instead of rustc's 16, which builds small programs about 20% faster on 4 cores. Not done, a possible future option behind a flag: rustc's incremental mode keyed by source file makes a rebuild after an edit about 28% faster but a first build about 22% slower, and most agent runs are first builds.
- **Tiered builds** (`tier.rs`). `run` and `test` without `--release`, with the build cache on, build a program whose opt-level 1 binary is not in the build cache at opt-level 0 (about 0.35 s instead of 0.85 s on bench/latency) and run it. The same Rust is then built at opt-level 1 by a detached, low-priority `brevi __tier` job into its own `.tmp-tier-KEY` folder under the build cache, and renamed into place under the key a plain opt-level 1 build uses. The child build of a cached run prints the path it will land at; the result cache keeps it with the entry (`upgrade`), and a replay swaps the optimised binary in once it is there. At most one job (two with 8 or more cores) runs per machine and user: each holds a file lock on a slot in `/tmp/brevi-tier-UID`, released when it exits or dies. No job starts with under 2 GB of memory available (Linux), and the job kills its rustc after five minutes. Without a free slot the quick binary is kept, and a later run tries again. `build` is never tiered. `BREVI_TIERED=0` turns it off; the test runner and CI set it, and the tiered-builds check turns it back on for itself.
- **Error mapping.** `report` reads `rustc --error-format=json`, maps each span back to a source line through the `/*@Ln*/` markers in the generated code, rewrites Rust words into Brevi words, and prints one short JSON line per error with a fix where one is known (`diag.rs`). For Python files these errors are held back until the dynamic build has had its turn.
- **Flags.** `rustc --edition 2021 -C overflow-checks=on`, debug info stripped (runtime errors carry source line numbers instead).

## Stage 5: running

> **Brevi native**, unless the CPython fallback below is used, which is **not native**.

`brevi run FILE a b` runs the built program with the arguments after the file (as `python3 FILE a b` does; the first word after FILE that is not a Brevi option starts them, and anything after `--` is the program's too) and exits with its status. A runtime error prints one JSON line: `{"e":"runtime","line":N,"msg":"..."}`, with the message in Python's words for Python files and `"type":"Class"` after the line when the error was raised as a class.

For `brevi run x.py` without `--native`, Brevi runs a strict build of itself as a child process. If that fails, it runs `python3 x.py` (or `$BREVI_PYTHON`) and prints a `{"path":"cpython",...}` line on stderr naming the line, function and error that stopped Brevi, so an agent always gets a result and knows it ran on CPython. If there is no Python either, it prints Brevi's errors.

## Other pieces

| File | Role |
| --- | --- |
| `gui.rs`, `gui_glue.rs`, `runtime/brevi_tk` | tkinter programs compiled onto the `brevi_tk` runtime. Headless by default (prints the window as text, reads actions from stdin); `--window` draws real windows with egui. |
| `diag.rs` | The JSON error format: `{"line":..,"msg":..,"fix":..}`, plus machine-applicable edits used by autofix and `brevi fix`. |
| `patch.rs` | `brevi patch`: edit by whole items, so a retry costs only the change. |
| `mcp.rs` | `brevi mcp`: keeps the program in the server so tools can send only changes. |
| `mcp_http.rs` | `brevi mcp --http`: each POST one JSON-RPC message, passed to a `brevi mcp` child over stdio, with the token, Origin and body-size checks. |
| `json.rs` | Minimal JSON reader for rustc's error output. |
| `proc.rs` | Starts the programs Brevi builds and stops whole process trees (`brevi run`, `brevi mcp`); holds the `--limit` wall-clock, output and thread limits. `LIMIT_PLATFORMS` is the one table of where each limit holds; a limit a platform cannot enforce is refused, never half held. On Windows (beta from v0.27.0) a `mem` limit is a Job Object the program starts suspended in; `cpu`, `file`, `fds` and `threads` are refused there ([SUPPORT_MATRIX.md](SUPPORT_MATRIX.md)). |
| `bvpy.rs` | What in a Python-style `.bv` file is Brevi's and not Python's; the line-for-line rewrite to Python for the dynamic build. |
| `pystd.rs`, `pygen.rs` | Standard-library functions for the typed build of a `.py` file, written in Python over the typed runtime; endless generators read only with `next()`. |
| `scoped.rs` | The cheap-to-copy name table type inference uses for its environments. |
| `unicode_names.rs` | Python's `\N{NAME}` escape for both lexers (table from `tools/unicode_names.py`). |
| `pynames/` | CPython's builtin, method, module and core names, which `pydiag.rs` reads to tell a Brevi spelling from Python's. |
| `grammar/` | A Lark grammar of Brevi for constrained decoding; CI parses every test program with it. |

## Where to look when

| Question | Start in |
| --- | --- |
| Why did my Python file build dynamically? | `BREVI_GUARD=print`, then `BREVI_DYN=0 brevi build x.py` to see the typed errors |
| What Rust did Brevi write? | `brevi emit x.py` (typed) or `BREVI_DYN=1 brevi emit x.py` (dynamic) |
| Why did `brevi run` use CPython? | The `{"path":"cpython"}` line on stderr names the first error and its function; `brevi run --native x.py` shows them all |
| A wrong answer compared with CPython | `tools/parity_probe.py`; typed fixes go in `infer.rs`/`emit.rs`/`prelude.rs`, dynamic ones in `dynpy/rt_*.rs` |
| A new Python library | `pybridge.rs` (`NATIVE` list decides what stays native); for `.bv` files also `engine.rs` (`OWN`) |

## Tests

- `tests/check.sh`: every program in `tests/run` and `tests/errors` must match its `.expected` file; `tests/bv_libs/*.bv` must run their libraries on the engine (never CPython); `tests/dyn/*.py` run with `BREVI_DYN=1` against `python3`'s output; `tests/fallback` covers the CPython fallback.
- `tools/conformance.py --check`: CPython's own test files, run natively; a wrong answer not listed in `compat/conformance_known_wrong/` (one file per entry, each with an issue, an owner and an expiry date) fails the check, and `--release-gate` holds a release to an absolute bar (core 90%, stdlib 80%, no known-wrong result): release.yml runs it on the commit it ships and publishes nothing below it.
- `tests/strict/`: `--strict` refuses every fallback (from the next release).
- `bench/verify.sh OLD NEW`: every saved benchmark solution must still give the same output.
- `tools/compat.py check`: every solution in `compat/baseline.txt` must keep building on new Rust and Python releases.
- `sh tools/ci_local.sh`: the PR checks, run locally before merging.

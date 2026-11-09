# Using Brevi
<!-- description: Brevi commands and flags: run, build, check, test; release, native and standalone builds; errors as JSON, the CPython fallback, the sandbox and GUI test mode. -->

Brevi reads plain Python (`.py`) and its own language (`.bv`) with the same commands. Most people, and most agents, only need `brevi run`.

## Quick start

```
brevi run main.py < input.txt          # build natively (cached), then run
brevi run main.py -- --verbose a.txt   # arguments after -- go to the program
brevi check main.py                    # build without running: errors only
brevi build main.py -o app --release   # an optimised binary to keep or ship
```

A file can import its sibling `.py` files and packages as usual (`import util`, `from models import Order`); Brevi joins them into one program, and errors name the right file.

## Commands

| Command | What it does |
| --- | --- |
| `brevi run FILE [-- ARGS]` | Build, then run with the arguments after `--`. Exits with the program's status. For `.py` files, falls back to `python3` when no native build works (see [below](#the-cpython-fallback)). |
| `brevi build FILE [-o OUT]` | Build a native program and write it to `OUT` (default: the file name without its extension). Never falls back. |
| `brevi check FILE [--fast]` | Build without running and print the errors, or nothing. `--fast` runs only Brevi's own checks, in milliseconds, without `rustc`. |
| `brevi test FILE` | Run the `test "name":` blocks of a Brevi program. |
| `brevi verify FILE.py [--input PATH]... [--mcp] [-- ARGS]` | Build natively, run the program and `python3` on the same inputs, and print a certificate when the two agree: stdout, exit code and stderr. See [Verify against CPython](#verify-against-cpython). |
| `brevi emit FILE` | Print the Rust that Brevi generates. |
| `brevi fix FILE` | Apply every suggested fix to a `.bv` file in place. |
| `brevi patch FILE PATCH` | Replace each `fn`, `type`, `enum` or `test` in `PATCH` by name in `FILE`, or add it; `del fn name` removes one. |
| `brevi guide [WORD]` | Print the Brevi language guide built into the compiler, or only the lines about one name or section. |
| `brevi mcp` | Serve the MCP tools over stdio (see [For AI agents](/docs/agents#mcp-server)). |
| `brevi --version` | Print the version. |

## Flags

| Flag | Commands | Effect |
| --- | --- | --- |
| `--release` | run, build | Optimise fully (rustc opt-level 3, the runtime inlined into the program). Builds take a few seconds longer; programs often run up to twice as fast as with the default build. Use it for a binary you keep. |
| `--native` | run | Never fall back to CPython: print Brevi's errors instead. `BREVI_NATIVE=1` does the same. |
| `--standalone` | build | For Python files that import libraries: pack CPython, the standard library and the imported packages into the binary, so it runs on a Linux machine with no Python. Linux only. |
| `--sandbox` | run, check, test | Refuse file access, native code and Python libraries unless the caller grants them (see [Sandbox](#sandbox)). |
| `--allow read,write,net,native` | run, check, test | Under the sandbox, grant these capabilities (implies `--sandbox`). |
| `--allow-read DIRS`, `--allow-write DIRS` | run, check, test | Under the sandbox, let the program read (or write) files only inside these folders or files, such as `--allow-read data --allow-write out` (implies `--sandbox`; from v0.26.0). |
| `--allow-net HOSTS` | run, check, test | Under the sandbox, let the program reach only these hosts over the network, such as `--allow-net api.openai.com,localhost` (implies `--sandbox`; from v0.26.0). |
| `--allow-py MODULES` | run, check, test | Under the sandbox, let CPython run these Python libraries only, such as `--allow-py yaml,pandas` (implies `--sandbox`). |
| `--limit cpu=S,mem=MB,time=S,output=KB,file=MB,...` | run, test, verify | Stop the program at the first resource limit it reaches (see [Resource limits](#resource-limits)). Works with or without the sandbox. |
| `--traceback` | run | When run falls back to CPython on a sure mistake, show CPython's whole traceback instead of its last line. `BREVI_TRACEBACK=1` does the same. |
| `--window` | run, build | tkinter programs open a real window instead of running headless (see [GUI programs](#gui-programs)). |
| `--explain` | any | Under each error, add the lines of the guide about the names it mentions. For Python files, also print [which path ran](#which-path-ran) even when it is the typed one, with every reason. |
| `--fast` | check | Brevi's own checks only, without `rustc`. |
| `--no-cache` | check, build, run | Build from scratch: skip the result cache and the build cache (`BREVI_NO_CACHE=1` does the same). |
| `-o OUT` | build | Where to write the binary. |

## How a Python file is built

Brevi tries the fastest way first and only moves on when it cannot match CPython:

1. **Typed build.** Brevi works out the type of every value and writes plain Rust. This is where the big speedups come from, and it covers most programs.
2. **Dynamic build.** If the program changes a value's type, mixes types in a list, or uses something the typed build cannot match exactly, Brevi builds it over dynamic values that behave like CPython's objects. Still native, no CPython involved, with [one line](#which-path-ran) saying why the typed build was not used.
3. **Library calls.** Imports Brevi does not build itself (pandas, yaml, ...) run in CPython loaded into the native program; the rest of the program stays native.
4. **CPython fallback.** Only `brevi run`, only when nothing above can build the file.

[Architecture](/docs/architecture) explains each step in detail.

## Which path ran

For a Python file, `brevi run`, `build` and `check` print one JSON line on stderr whenever the file is off the typed path, so you (or your agent) know what ran and what to change to get the fast path back:

```
{"path":"dynamic","line":12,"fn":"parse","why":"rows holds [int] (set on line 9) but is set to [str] here"}
{"path":"pybridge","why":"numpy run in CPython","py":["numpy"]}
{"path":"cpython","line":2,"fn":"g","why":"Brevi could not build it: unknown function eval()","by":"python3"}
```

`path` is `typed`, `pybridge` (typed, with library calls in CPython), `dynamic` (native over dynamic values) or `cpython` (the fallback below). `line` and `fn` point at the code that kept the file off the typed path; rewriting that one function is usually enough. The typed path prints nothing. `--explain` adds the suggested fix and the other reasons; `BREVI_PATH=1` prints the line for every path, `BREVI_PATH=0` never. The MCP tools return the same report as structured data.

## The CPython fallback

`brevi run x.py` always gives a result. When Brevi cannot build a file natively, it runs it with `python3` (or `$BREVI_PYTHON`) and prints one line on stderr naming the first line that stopped it:

```
{"path":"cpython","line":2,"why":"Brevi could not build it: expected ')', found end of line","by":"python3"}
```

A sure mistake (see [Errors](#errors)) is named in that line the same way. CPython's traceback for it is then cut to its last line, the error itself, with the file and line it was raised at (from v0.24.2), so the error is not read twice:

```
{"path":"cpython","line":3,"why":"Brevi could not build it: name 'totl' is not defined","by":"python3"}
NameError: name 'totl' is not defined (main.py line 3)
```

Pass `--traceback` (or set `BREVI_TRACEBACK=1`) to see the whole traceback. Other fallbacks show CPython's output as it is. A sandboxed program never falls back.

Use `brevi run --native`, `brevi build` or `brevi check` when you need to know a file builds natively: they never fall back, and print the errors instead.

## Verify against CPython

`brevi verify x.py` (from v0.26.0) is proof that the native build answers as CPython does. It builds the file natively (strict, as `brevi build` does: no fallback), then runs `python3` (or `$BREVI_PYTHON`) and the built program on the same inputs, and compares stdout byte for byte, the exit code and stderr. CPython's traceback frames are left out of the stderr comparison, since a built program names the exception but not the frames. CPython runs each input twice: output that changes from run to run (set order, the time, unseeded `random`) cannot be certified, and is reported as `unstable` instead of matched by luck.

```
$ brevi verify main.py --input tests/ -- --verbose
{"case":1,"input":"tests/1.in","same":true,"cpython_ms":38,"brevi_ms":2}
{"case":2,"input":"tests/2.in","same":true,"cpython_ms":41,"brevi_ms":2}
{"verify":"identical","file":"main.py","sha256":"15532f5a...","brevi":"0.26.0","python":"3.13.13","path":"typed","inputs":[{"input":"tests/1.in","sha256":"7de1555d..."},{"input":"tests/2.in","sha256":"10159baf..."}],"args":["--verbose"],"compared":"stdout, exit code and stderr, CPython run twice","differ":0,"unstable":0,"timeout":0,"limit":0,"cpython_ms":79,"brevi_ms":4,"speedup":19.8}
```

- `--input PATH` feeds a file to stdin, one run per `--input`; a folder gives one run per file in it (only its `*.in` files when it has any). With no input the program runs once with empty stdin. Arguments after `--` go to both.
- The last line is the certificate: the verdict, the SHA-256 of the file and of every input, the Brevi and Python versions, which [path](#which-path-ran) built it, and the time each side took. A CI job or an agent can keep it, and gate on the exit status.
- Verdicts: `identical` (exit 0); `differs`, a wrong answer from Brevi, with the first line that differs (`"on":"stdout"`, `"exit code"` or `"stderr"`); `unstable`; `timeout` (`--timeout SECS`, default 60 per run); `limit` (a `--limit` stopped one side); `not built`, with Brevi's errors above it. Every verdict but `identical` exits 1; a usage error exits 2.
- An MCP server is verified with a client session: when the file imports `mcp`, a `.jsonl` input (or any input, with `--mcp`) holds one JSON-RPC message per line, and each request is sent once the reply to the one before it is out, so the answer does not depend on timing. `#nowait MSG` sends a request without waiting (to cancel it, or to leave it running when the client hangs up), `#wait N` waits for N more lines, `#raw TEXT` sends a line that is not JSON, and `# ...` is a comment.
- `--limit cpu=2,mem=512,...` holds both CPython and the native build to the same [resource limits](#resource-limits). An input on which either side reaches one gives the verdict `limit`, which certifies nothing, as `timeout` does.
- `--once` runs CPython once instead of twice. `--release` verifies the fully optimised build.
- The program runs three times per input, so a program that writes files sees its own earlier runs.
- `--sandbox` is refused: verify runs the code under CPython, which cannot be held to the sandbox's grants. Verify only code you would run with `python3`.

Please report any `differs` at [github.com/ets-public/brevi/issues](https://github.com/ets-public/brevi/issues). How often Brevi gives a wrong answer on CPython's own test suite is measured for every release ([CPython conformance report](https://github.com/ets-public/brevi/blob/main/bench/conformance/report.md)).

## Errors

Every error is one line of JSON on stderr, made for a model to read cheaply.

**Build errors** name the line, say what is wrong in plain words and, where Brevi knows it, give the fix:

```
{"line":2,"msg":"expected ')', found end of line","fix":"insert ')'"}
{"line":3,"msg":"cannot add str and int","fix":"turn the number into text with str(x), or write \"{x}\""}
```

There is no file path, column, error code or Rust in them. In a program over several files, errors add `"file"`. Anything `rustc` rejects is mapped back to the source line and rewritten in Brevi's words. In `.bv` files a small typo of a known name (`totl` for `total`) is fixed during the build with a one-line note instead of failing.

**Runtime errors** are one line too, with the source line:

```
{"e":"runtime","line":2,"msg":"index 5 out of range for length 1"}
```

A Python file exits as CPython does: status 1 after an uncaught exception, and its own status after `sys.exit(n)`. A `.bv` program exits with status 101 after a runtime error like the one above.

**Sure mistakes in Python files are reported before the program runs.** A name that is never defined, a method no Python type has, and adding text to a number stop `check` and `build` with one error line and a hint. Code that may never run is left alone (branches under `sys.version_info`, `sys.platform`, `TYPE_CHECKING` or `if False`, and `except` blocks). `brevi run` falls back to python3 instead and names the mistake in its path line:

```
{"line":3,"msg":"name 'totl' is not defined","fix":"did you mean total?"}
{"line":2,"msg":"no Python type has a method .uppr()","fix":"did you mean .upper()?"}
{"path":"cpython","line":3,"why":"Brevi could not build it: name 'totl' is not defined","by":"python3"}
```

Other mistakes that Python only finds when the line runs (an index out of range, a call with too few arguments, a failed `int("x")`) are left to run time: the program raises Python's own exception at that point, as CPython would. In Brevi's own `.bv` language every type mistake is reported before the program runs. To see everything Brevi's typed checks think of a Python file, run `BREVI_DYN=0 brevi check x.py`.

## Sandbox

**What it is, and what it is not.** The sandbox is a cheap capability and limit layer for agent code you mostly trust. It is not an isolation boundary for hostile code. Brevi's own generated code enforces it: there is no seccomp filter, namespace or VM around the program, so the guarantee rests on Brevi being right. `--allow-py` and `--allow native` are full trust, Landlock holds CPython to the file grants on Linux 5.13 and later only, a file check and the open that follows are two steps, and no outside security review has been done yet. Run untrusted code inside a container or VM as well, and use the sandbox as a second layer inside it.

With `--sandbox` (and in the MCP tools, which sandbox by default), **only the caller grants capabilities**: `--allow read,write,net,native`, `--allow-net api.example.com` to let the program reach just those hosts, `--allow-read data` and `--allow-write out` to let it read or write files only inside those folders, or `--allow-py yaml,pandas` to let CPython run just those libraries (the MCP tools' `allow`, `allow_net`, `allow_read`, `allow_write` and `allow_py` arguments). The code being sandboxed cannot grant itself anything: an `allow` line in it that asks for more than the caller granted is refused (from v0.24.6; before it, an allow line in the file was enough).

```
$ brevi run --sandbox fetch.py
{"line":1,"msg":"import httpx (the network) needs allow net","fix":"do without it, or the caller grants it: --allow-net HOST,... for some hosts or --allow net for all (MCP tools: \"allow_net\": [\"HOST\"] or \"allow\": [\"net\"])"}
$ brevi run --allow-net api.example.com fetch.py
```

`requests` and `httpx` are Brevi's own (see [requests and httpx](/docs/compatibility#requests-and-httpx)), so they need only the network, not CPython. With `--allow-net`, a connection to any host the caller did not name fails when the program makes it, as a `PermissionError` inside the library's own error (`httpx.ConnectError`, `requests.ConnectionError`): `brevi sandbox: the caller did not grant the network to evil.example (--allow-net evil.example, or --allow net for every host)`. Hosts are matched by the name in the URL (an IP address is its own name), and proxies from the environment are not used under the sandbox, which reads no environment.

From v0.26.0, `--allow-read DIR` and `--allow-write DIR` (comma-separated folders or files, which must exist) hold file access to those folders: `--allow read` grants every file the user can read, `--allow-read data` only what is under `data`. Brevi checks each file the program opens by its real path, with `..` and symlinks resolved, so a symlink inside the folder that points outside, or `data/../secret`, is refused with a `PermissionError` (`brevi: --sandbox: 'data/../secret' is outside the folders granted to read (--allow-read)`), and so is a name outside the folders that does not exist. This holds for `open()`, polars, safetensors, sqlite3 (`ATTACH` included), duckdb (its SQL reaches files only in folders granted both read and write), onnxruntime (a model with external data files needs `--allow read`, as ONNX Runtime opens those itself) and the CA files of `requests` and `httpx`. CPython code (under `--allow-py` or `--allow native`) is held to the same folders by the kernel, with Linux Landlock: it can read CPython's own files and a few system files, nothing else outside the grants, and with no read or write grant it can open no file of yours (a `PermissionError`). Without Landlock (macOS, Windows, Linux before 5.13), a run that names folders and loads CPython is refused. In the MCP tools, `allow_read` and `allow_write` paths are relative to the folder `brevi mcp` was started in.

`--allow-py` grants CPython for the named modules, and so whatever CPython can reach through them (the network, other processes), except files: on Linux those stay held to the read and write grants, as above. So `--allow-py` alone no longer lets a program read your files (from v0.26.0); add `--allow read` or `--allow-read DIR`. It does not grant `rust fn`, crates or any other library.

Outside the sandbox, a program can declare what it touches with an `allow` line at the top, and is then held to it. In a Python file write it as a comment, so the file stays valid Python; in a `.bv` file write it bare:

```
# allow read, write        (Python file)
allow read, write          (.bv file)
```

A comment counts only when it holds nothing but capability names, so `# allow me to explain` stays an ordinary comment. The bare form still works in a `.py` file, but CPython then refuses the file.

| Capability | Covers |
| --- | --- |
| `read` | reading files; `--allow-read DIR` grants it for those folders only |
| `write` | writing files; `--allow-write DIR` grants it for those folders only |
| `net` | the network, through Brevi's `requests` and `httpx`; `--allow-net` grants it for named hosts only |
| `native` | `rust fn`, `py fn`, `rust:` and `py:` blocks, crates, and Python libraries that CPython runs (`subprocess`, `numpy`, `requests`, file and process calls of `os` and `pathlib`, ...), which can do anything |
| `none` | nothing beyond stdin, stdout and arguments |

stdin, stdout, program arguments and `use "file.bv"` are always allowed. Under `--sandbox` a program that uses something the caller did not grant, or (without `--sandbox`) one with an `allow` line that uses something it did not declare, is refused at build time:

```
{"line":1,"msg":"read() needs allow read","fix":"do without it, or the caller grants it: --allow read (MCP tools: \"allow\": [\"read\"])"}
```

Some modules work without `allow native`, because Brevi builds its own copy of the part that touches nothing (from v0.24.2):

- **Path text:** `os.path.join`, `basename`, `dirname`, `split`, `splitext`, `normpath`, `normcase`, `isabs`, `commonprefix`, `os.sep` and the other path constants, and `pathlib.PurePath` / `PurePosixPath` (`/`, `name`, `stem`, `suffix`, `parent`, `parts`, `with_suffix`, `relative_to`, ...). They follow POSIX rules, as CPython does on Linux and macOS. `os.listdir`, `os.walk`, `os.path.exists`, `isfile`, `isdir`, `islink` and `getsize` are Brevi's own too (from v0.26.0) and need only the read grant (`--allow read` or `--allow-read DIR`). Any other use of `os` or `pathlib` (`os.environ`, `os.system`, `os.remove`, `pathlib.Path`, or `os` passed around as a value) is refused with the name of the call:

  ```
  {"line":3,"msg":"os.system needs allow native","fix":"without it only path functions run (os.path.join, basename, dirname, splitext, normpath, os.sep, pathlib.PurePath); do without it, or the caller grants it: --allow-py os (MCP tools: \"allow_py\": [\"os\"])"}
  ```

- **io:** `io.StringIO`, an in-memory text file (so `csv.writer(io.StringIO())` works too). Other `io` names (`io.BytesIO`, `io.open`) are refused by name.

- **textwrap:** `wrap`, `fill`, `shorten`, `dedent`, `indent` and `TextWrapper`.

- **argparse:** everything but `argparse.FileType` and `fromfile_prefix_chars` (both open files). Help text is 80 columns wide under the sandbox, which reads no environment; outside it, it follows `$COLUMNS` or the terminal's width, as in CPython.

- **pydantic** (from v0.26.0): Brevi's own copy needs nothing granted; what it lacks (see [pydantic](/docs/compatibility#pydantic)), such as `field_serializer`, is refused by name unless the caller grants `--allow-py pydantic`.

- **requests and httpx** (from v0.26.0): Brevi's own copies, over native sockets and TLS; they need `allow net` (or `--allow-net` for some hosts), not `allow native`.

- **jinja2** (from v0.26.0): Brevi's own jinja2 and markupsafe need nothing granted; what they lack (see [jinja2](/docs/compatibility#jinja2)), such as `FileSystemLoader`, is refused by name unless the caller grants `--allow-py jinja2`.
- **polars** (from v0.26.0): Brevi's own polars needs `allow read` to read a file and `allow write` to write one (text in memory needs nothing); what it lacks (see [polars](/docs/compatibility#polars)), such as Parquet or date columns, is refused by name unless the caller grants `--allow-py polars`.
- **sqlite3 and duckdb** (from v0.26.0): Brevi's own copies need `allow read` to open a database file and `allow write` to change one (`:memory:` needs nothing; a read-only open needs only `allow read`). DuckDB SQL that reads or writes other files (`read_csv`, `COPY`, `ATTACH`) needs both grants; extensions never load. What they lack (see [sqlite3 and duckdb](/docs/compatibility#sqlite3-and-duckdb)), such as `create_function` or data frames, is refused by name unless the caller grants `--allow-py sqlite3` or `--allow-py duckdb`.
- **numpy** (from v0.26.0): Brevi's own numpy needs nothing granted: it has no file, network or process access (`np.save`, `np.load`, `tofile` and the text readers are not in it). What it lacks (see [numpy](/docs/compatibility#numpy)), such as `np.linalg.eig` or `float16`, is refused by name unless the caller grants `--allow-py numpy`.
- **safetensors and onnxruntime** (from v0.26.0): Brevi's own copies need `allow read` to read a weights or model file and `allow write` for `save_file`, an optimized model, a profile, or a session config entry naming a file (which needs both). A model given as bytes needs nothing, unless its weights are in external data files. What they lack (see [safetensors and onnxruntime](/docs/compatibility#safetensors-and-onnxruntime)), such as torch tensors or `OrtValue`, is refused by name unless the caller grants `--allow-py safetensors` or `--allow-py onnxruntime`.
- **mcp** (from v0.26.0): Brevi's own copy of the MCP SDK; a stdio server needs no grant, and one served over HTTP needs the network grant for the host it listens on (`--allow-net 127.0.0.1`). What it lacks (the Starlette apps, sampling, elicitation; see [mcp](/docs/compatibility#mcp-mcp-servers)) is refused by name unless the caller grants `--allow-py mcp`.
- **anthropic and openai** (from v0.26.0): Brevi's own copies of the SDKs, over its httpx; they need `allow net` too (`--allow-net api.anthropic.com` or `--allow-net api.openai.com`), not `allow native`. What they lack (see [anthropic and openai](/docs/compatibility#anthropic-and-openai)) is refused by name unless the caller grants `--allow-py anthropic` or `--allow-py openai`.

- **random:** `seed`, `random`, `randint`, `randrange`, `choice`, `choices`, `shuffle`, `sample`, `uniform`, `gauss`, `getrandbits` and `random.Random`, using CPython's own generator, so a seeded program draws the same numbers as in CPython. Seeds must be ints. Other functions (`random.betavariate`, ...) give one error naming them.

The check runs before every build path, and a refusal is final. A refused program is never built over dynamic values, never run through a library call, and never run with python3. A sandboxed program never falls back to CPython for any reason, because CPython cannot enforce `allow` lines. If Brevi cannot build it, you get the build errors and a note saying so. Without `--allow native` or `--allow-py`, the library bridge is off completely, so no path from the program reaches CPython; with `--allow-py`, only the named modules do.

The MCP tools sandbox by default (pass `sandbox: false` to turn it off). Outside the sandbox the path-text, `io.StringIO`, `textwrap`, `argparse` and `random` copies are used too, whenever a program needs nothing more of those modules (random's generator core is native).

### Resource limits

The sandbox decides what a program may touch; `--limit` decides how much it may use (from v0.26.0). The caller sets the limits, as with grants, and the program is stopped at the first one it reaches:

| Limit | Unit | Holds | How |
| --- | --- | --- | --- |
| `cpu` | seconds | CPU time, all threads together | the kernel (`RLIMIT_CPU`) |
| `mem` | MB | address space, Linux only | the kernel (`RLIMIT_AS`) |
| `time` | seconds | wall-clock time, waiting included | `brevi`, which stops the program |
| `output` | KB | stdout and stderr together | `brevi`, which passes output on and stops the program at the limit |
| `file` | MB | the size of any one file the program writes | the kernel (`RLIMIT_FSIZE`) |
| `written` | MB | bytes written in all, to files, pipes and sockets (stdout and stderr not counted), Linux only | the kernel's count (`/proc/PID/io`), which `brevi` checks as the program runs |
| `files` | count | files the program creates | Brevi's own file code in the program; `--sandbox` without `--allow-py` or `native` only |
| `net` | MB | bytes sent and received over the network | Brevi's own network code in the program; `--sandbox` without `--allow-py` or `native` only |
| `threads` | count | threads at once, Linux only | the kernel's count (`/proc/PID/status`), which `brevi` checks as the program runs |
| `fds` | count | files and sockets open at once | the kernel (`RLIMIT_NOFILE`): an open past it fails with `OSError: Too many open files`, it does not stop the program |

Plain numbers use those units; `mem=1gb`, `output=512kb`, `time=1.5` and `time=500ms` work too. Join limits with commas:

```
$ brevi run --sandbox --limit cpu=2,mem=512,time=10,output=64 agent_code.py
...
{"e":"limit","on":"cpu","limit":"2 s","msg":"the program used up its 2 s of CPU time","fix":"raise it with --limit cpu=... if the program needs more"}
$ echo $?
124
```

A program stopped by a limit exits with status 124 and its last stderr line is one JSON line naming the limit (`"on"` names it: `cpu`, `mem`, `time`, `output`, `file`, `written`, `files`, `net` or `threads`). Output written before the limit is kept (with `output`, exactly up to it); output a program had buffered but not yet written when it was stopped is lost.

- The kernel limits are set in the program's own process before it starts, so nothing in it can raise them, CPython loaded through `--allow-py` included. Memory counts address space, not resident memory: CPython and numpy reserve more than they touch, so give a program that loads them a few hundred MB.
- The limits hold the program, not the build: a cold first build can take longer than `time`. `brevi build` and `brevi check` refuse `--limit`.
- With `output` (or `mem`, which reads stderr to tell an out-of-memory stop from a crash) the program's stdout or stderr is a pipe, not the terminal.
- `written` and `threads` are checked every few milliseconds, so a program can pass them by what it does in that time before it is stopped. `files` and `net` are counted inside the program by Brevi's own file and network code (`open`, polars, sqlite3, duckdb, safetensors, onnxruntime, requests, httpx and the SDKs over it), which is why they need `--sandbox` without `--allow-py` or `--allow native`: CPython and native code would not be counted. Under `files`, DuckDB's SQL reaches no files besides the database.
- A limit Brevi cannot enforce on the machine is refused, never ignored: `mem` on macOS, `written` and `threads` outside Linux, and `cpu`, `mem`, `file` and `fds` on Windows, give an error and the program does not run.
- In the MCP tools `run`, `test` and `verify` take `limits: {"cpu": 2, "mem": 512, "time": 10, "output": 64, "file": 10}` (and `written`, `net`, `threads`, `fds`, `files`) (each `cases` entry is held to them separately). The call's `timeout` still covers build and run together.

What the sandbox and the limits protect against, and what they do not, is in the [threat model](https://github.com/ets-public/brevi/blob/main/docs/THREAT_MODEL.md).

> This is the behaviour from v0.24.1. In v0.24.0 and earlier the check covered only Brevi programs and typed Python builds: a Python file that fell to the dynamic build, a library call or the CPython fallback was not held to it. Whatever the version, running code you do not trust inside a container or VM is still wise.

## GUI programs

tkinter programs build too: widgets, variables, callbacks, key and mouse bindings, `after` timers, canvas and message boxes, including apps built around a class (`command=self.on_click`). They run on Brevi's own `brevi_tk` runtime.

By default a GUI program runs **headless**, so it can be tested like any other program, by a script or by a model that cannot see a screen. `mainloop()` reads actions from stdin, one per line, and `show` prints the window as text:

```
$ printf 'show\nclick Go\nshow\n' | brevi run app.py
window "tk"
  Label "hi"
  Button "Go"
window "tk"
  Label "clicked"
  Button "Go"
```

| Action | Effect |
| --- | --- |
| `show` | print every visible widget, indented under its parent |
| `click TEXT` | click the button, checkbutton or radiobutton showing TEXT |
| `type N TEXT` | replace the text of the Nth text box (Entry, Text, Spinbox, Combobox; from 1) |
| `key N KEY` | press KEY (`Return`, `Escape`, `a`, ...) in the Nth text box (0: the window) |
| `select N I` | select line I (from 0) of the Nth list box |
| `slide N X` | move the Nth slider to X |
| `tap N X Y` | click the Nth canvas at X, Y |
| `wait MS` | let MS milliseconds pass (runs `after` timers) |
| `answer yes\|no\|TEXT` | the answer to the next dialog (default: yes / ok) |
| `close` | close the window |

`brevi build --window app.py` gives a real window, drawn with egui (eframe 0.37, OpenGL). The first window build downloads and compiles eframe, about a minute, then it is cached. On Linux the window needs an X11 or Wayland display, OpenGL and `libxkbcommon-x11`; without a display, or with `BREVI_GUI=headless`, the same binary runs headless.

## Libraries

- **Python packages** in Python files: `import numpy as np`, `pandas`, `yaml`, `pathlib`, `subprocess` or any other installed module works. Its calls run in CPython inside the native program, so results are CPython's own. The program needs `libpython` where it runs, unless built with `--standalone`.
- **Standalone binaries**: `brevi build x.py --standalone` carries CPython, the standard library and the imported packages (with what they require). The first run unpacks them once into `~/.cache/brevi/py`, about 1 to 3 seconds; later runs start at normal speed. Expect about 13 MB for a standard-library program and 30 to 45 MB with numpy or pandas. Linux only.
- **Rust crates and Python code from Brevi programs**: `use sha2 "0.10"` with a `rust fn`, or a `py fn` with a Python body. See [Libraries](/docs/libraries).

## Speed and the build cache

Running `brevi check`, `build` or `run` again on files that did not change replays the last result: the same output, the same `{"path":...}` line and the same binary, in about 10 ms, with no parsing, type inference or `rustc`. The key is a hash of the program's files (every `.py` and `.bv` file in its folder and the package folders it imports), the command and flags, the `BREVI_*` and Python settings in the environment, the Brevi binary and the Rust toolchain; any change to one builds again. A failed build is replayed too when the program imports only its own files and Python's standard library (a later `pip install` must not meet an old refusal). Below that, builds are also cached by the generated code, so an edit that changes nothing in the generated Rust skips `rustc`. The cache keeps at most `BREVI_CACHE_MB` megabytes (default 1024), least recently used first out; `--no-cache` builds from scratch. A new small program builds in well under a second by default (about 0.15 to 0.3 s) and a few seconds with `--release`. The very first build after installing Brevi, or after a Rust update, also builds Brevi's runtime library once, which takes about 5 to 15 seconds.

Write the plain, direct program. Brevi runs plain loops over ints and lists several times faster than CPython (1.7x to 39.5x, median 8.5x, on our 12 compute benchmarks), so code hand-tuned for CPython (bit tricks, packing values into big integers) is usually slower under Brevi, not faster.

## Environment variables

| Variable | Effect |
| --- | --- |
| `BREVI_NATIVE=1` | Same as `--native`: never fall back to CPython. |
| `BREVI_TRACEBACK=1` | Same as `--traceback`: show the whole CPython traceback after a fallback on a sure mistake. |
| `BREVI_PYTHON` | The Python used for the fallback (default `python3`). |
| `BREVI_CACHE` | Where builds and runtimes are cached (default `~/.cache/brevi`). |
| `BREVI_NO_CACHE=1` | Same as `--no-cache`: do not use the result or build cache. |
| `BREVI_CACHE_MB` | Most megabytes of builds the cache keeps (default 1024); the least recently used go first. |
| `BREVI_TIME=1` | Print how long each build stage took, in ms, on stderr. |
| `BREVI_PY_CACHE` | Where `--standalone` binaries unpack CPython (default `~/.cache/brevi/py`). |
| `BREVI_GUI=headless` | Run a `--window` GUI build headless. |
| `BREVI_DYN=1` / `BREVI_DYN=0` | Force the dynamic build, or turn it off (to see the typed build's errors). |
| `BREVI_PATH=1` / `BREVI_PATH=0` | Print [which path ran](#which-path-ran) for every build, including typed ones, or never. |
| `BREVI_GUARD=print` | Print why a Python file was sent to the dynamic build. |
| `BREVI_DYN_DEBUG=1` | Print `rustc` errors from generated dynamic code. |

## Writing Brevi itself

Brevi also has its own Python-like language for `.bv` files, shorter for a model to write and checked fully before it runs. You do not need it to build Python. A `.bv` file that is plain Python (it uses `def`, `class` or `import`, CPython could compile it, and it uses none of Brevi's own names or `"{n}"` strings) is built exactly as the same file saved as `.py`, except that it never falls back to python3. See the [Language guide](/docs/language).

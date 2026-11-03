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
| `--sandbox` | run, check, test | Refuse anything the program did not declare with an `allow` line (see [Sandbox](#sandbox)). |
| `--window` | run, build | tkinter programs open a real window instead of running headless (see [GUI programs](#gui-programs)). |
| `--explain` | any | Under each error, add the lines of the guide about the names it mentions. |
| `--fast` | check | Brevi's own checks only, without `rustc`. |
| `-o OUT` | build | Where to write the binary. |

## How a Python file is built

Brevi tries the fastest way first and only moves on when it cannot match CPython:

1. **Typed build.** Brevi works out the type of every value and writes plain Rust. This is where the big speedups come from, and it covers most programs.
2. **Dynamic build.** If the program changes a value's type, mixes types in a list, or uses something the typed build cannot match exactly, Brevi builds it over dynamic values that behave like CPython's objects. Still native, no CPython involved, and silent: you just get a program.
3. **Library calls.** Imports Brevi does not build itself (numpy, pandas, sqlite3, requests, ...) run in CPython loaded into the native program; the rest of the program stays native.
4. **CPython fallback.** Only `brevi run`, only when nothing above can build the file.

[Architecture](/docs/architecture) explains each step in detail.

## The CPython fallback

`brevi run x.py` always gives a result. When Brevi cannot build a file natively, it runs it with `python3` (or `$BREVI_PYTHON`) and prints one note on stderr naming the first line that stopped it:

```
note: ran with python3; Brevi could not build line 2: expected ')', found end of line
```

A sure mistake (see [Errors](#errors)) is named in the note the same way. A sandboxed program never falls back.

Use `brevi run --native`, `brevi build` or `brevi check` when you need to know a file builds natively: they never fall back, and print the errors instead.

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

The program exits with status 101 after a runtime error, and with its own status after `sys.exit(n)`.

**Sure mistakes in Python files are reported before the program runs.** A name that is never defined, a method no Python type has, and adding text to a number stop `check` and `build` with one error line and a hint. Code that may never run is left alone (branches under `sys.version_info`, `sys.platform`, `TYPE_CHECKING` or `if False`, and `except` blocks). `brevi run` falls back to python3 instead and names the mistake in its note:

```
{"line":3,"msg":"name 'totl' is not defined","fix":"did you mean total?"}
{"line":2,"msg":"no Python type has a method .uppr()","fix":"did you mean .upper()?"}
note: ran with python3; Brevi could not build line 3: name 'totl' is not defined
```

Other mistakes that Python only finds when the line runs (an index out of range, a call with too few arguments, a failed `int("x")`) are left to run time: the program raises Python's own exception at that point, as CPython would. In Brevi's own `.bv` language every type mistake is reported before the program runs. To see everything Brevi's typed checks think of a Python file, run `BREVI_DYN=0 brevi check x.py`.

## Sandbox

A program, Brevi or Python, can declare what it touches with an `allow` line at the top:

```
allow read, write
```

| Capability | Covers |
| --- | --- |
| `read` | reading files |
| `write` | writing files |
| `native` | `rust fn`, `py fn`, `rust:` and `py:` blocks, crates, and Python libraries that CPython runs (`import os`, `subprocess`, `numpy`, `requests`, ...), which can do anything |
| `none` | nothing beyond stdin, stdout and arguments |

stdin, stdout, program arguments and `use "file.bv"` are always allowed. With `brevi run --sandbox`, or as soon as a program has any `allow` line, a program that uses something it did not declare is refused at build time:

```
{"line":1,"msg":"read() needs allow read","fix":"add the line `allow read` at the top, or do without it"}
```

The check runs before every build path, and a refusal is final. A refused program is never built over dynamic values, never run through a library call, and never run with python3. A sandboxed program never falls back to CPython for any reason, because CPython cannot enforce `allow` lines. If Brevi cannot build it, you get the build errors and a note saying so. Without `allow native`, the library bridge is off completely, so no path from the program reaches CPython.

The MCP tools sandbox by default (pass `sandbox: false` to turn it off).

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

`brevi build --window app.py` gives a real window, drawn with egui.

## Libraries

- **Python packages** in Python files: `import numpy as np`, `pandas`, `sqlite3`, `pathlib`, `subprocess` or any other installed module works. Its calls run in CPython inside the native program, so results are CPython's own. The program needs `libpython` where it runs, unless built with `--standalone`.
- **Standalone binaries**: `brevi build x.py --standalone` carries CPython, the standard library and the imported packages (with what they require). The first run unpacks them once into `~/.cache/brevi/py`, about 1 to 3 seconds; later runs start at normal speed. Expect about 13 MB for a standard-library program and 30 to 45 MB with numpy or pandas. Linux only.
- **Rust crates and Python code from Brevi programs**: `use sha2 "0.10"` with a `rust fn`, or a `py fn` with a Python body. See [Libraries](/docs/libraries).

## Speed and the build cache

Builds are cached by the generated code, the flags and the Rust version, so running the same program again (re-running tests, an MCP call) skips `rustc` entirely. A new small program builds in well under a second by default (about 0.15 to 0.3 s) and a few seconds with `--release`. The very first build after installing Brevi, or after a Rust update, also builds Brevi's runtime library once, which takes about 5 to 15 seconds.

Write the plain, direct program. Brevi runs plain loops over ints and lists 10 to 50 times faster than CPython, so code hand-tuned for CPython (bit tricks, packing values into big integers) is usually slower under Brevi, not faster.

## Environment variables

| Variable | Effect |
| --- | --- |
| `BREVI_NATIVE=1` | Same as `--native`: never fall back to CPython. |
| `BREVI_PYTHON` | The Python used for the fallback (default `python3`). |
| `BREVI_CACHE` | Where builds and runtimes are cached (default `~/.cache/brevi`). |
| `BREVI_NO_CACHE=1` | Do not use the build cache. |
| `BREVI_PY_CACHE` | Where `--standalone` binaries unpack CPython (default `~/.cache/brevi/py`). |
| `BREVI_GUI=headless` | Run a `--window` GUI build headless. |
| `BREVI_DYN=1` / `BREVI_DYN=0` | Force the dynamic build, or turn it off (to see the typed build's errors). |
| `BREVI_GUARD=print` | Print why a Python file was sent to the dynamic build. |
| `BREVI_DYN_DEBUG=1` | Print `rustc` errors from generated dynamic code. |

## Writing Brevi itself

Brevi also has its own Python-like language for `.bv` files, shorter for a model to write and checked fully before it runs. You do not need it to build Python. See the [Language guide](/docs/language).

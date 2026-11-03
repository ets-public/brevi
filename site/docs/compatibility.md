# What works, and limits
<!-- description: Which Python Brevi builds natively, how libraries work, typed versus dynamic builds, big integers, and the known gaps. -->

Brevi's promise for a Python file is simple: **the native program prints exactly what CPython prints, or Brevi refuses to build it.** This page says how much Python that covers today and where the edges are.

## In numbers

| Check | Result |
| --- | --- |
| Saved agent programs (591, from benchmark runs) built unchanged | 591 build, 591 match CPython on every test |
| Large programs (18; chess, a text adventure, an interpreter; 200 to 750 lines over 3 to 4 files) | 18 build, 18 match |
| Behaviour traps (13: big ints, `//` and `%` signs, float printing, dict order, unicode, aliasing, deep recursion, ...) | 13 match |
| Release test (24 tasks, Sonnet and Haiku writing fresh Python) | 92 to 96% of final programs native, the rest via the CPython fallback |
| Random generated programs (bug hunt, 2 Nov 2026) | 400 of 400 match |

"Match" means stdout byte for byte and the same exit status. See [Benchmarks](/docs/benchmarks) for the method.

## Typed and dynamic builds

Brevi has two native ways to build a Python file, and picks between them on its own.

**Typed build**, the fast path. Brevi infers a fixed type for every value across the whole program and writes plain Rust. Most agent-written programs build this way. Python behaviour that shows in output is reproduced: unlimited integers, shared mutable objects (`r = rows[k]; r[0] -= 1` changes `rows`), dict printing and order, Python's float formatting and `round`, exception messages.

**Dynamic build**, the exact path. When a value changes type, a list mixes types, or the typed build would print something different from CPython (iterating a set, for example, where CPython's hash order shows), the whole program is built over a dynamic value type with CPython's own semantics: hashing and set order, formatting, exceptions, classes and method lookup. Still native, with no CPython involved, but slower than typed: typically somewhat faster than CPython rather than many times faster.

You do not choose; Brevi tries typed first. `BREVI_GUARD=print brevi build x.py` shows why a file went dynamic.

## Python features

Works in native builds:

- Functions, default and keyword arguments, `*args` and `**kwargs`, closures, `nonlocal` and `global`, lambdas, decorators
- Classes, inheritance and `super()`, operator methods (`__add__`, `__eq__`, `__lt__`, `__repr__`, ...), properties
- Comprehensions, generators and `yield`, iterators, `zip`, `enumerate`, `map`, `filter`, `sorted` with `key`
- `try` / `except` / `else` / `finally`, `raise`, custom exceptions, `with` blocks
- f-strings with format specs, `%` formatting, `str.format`
- The walrus operator, slicing, tuple unpacking, chained comparisons
- Multi-file programs: sibling modules and packages imported as usual
- Reading stdin, arguments, files (`open`, `pathlib` reads and writes)

Built natively by Brevi itself in the dynamic build: `sys`, `math`, `collections`, `heapq`, `bisect`, `functools`, `itertools`, `copy`, `operator`, `string`, `time`, `typing`, `dataclasses`, `abc`, `re`, `json`, `decimal`, `fractions`, `datetime`, `csv`, `html`, `statistics`, `threading`. The typed build covers most of the same, plus pieces of others.

Not built natively yet (`brevi run` falls back to CPython; `--native` refuses):

- `async` / `await` and `asyncio`
- The `match` statement
- `eval`, `exec` and other run-time code generation
- Modules outside the list above in a program that also needs the dynamic build (see below)

## Libraries (numpy, pandas, ...)

A Python file that imports a module Brevi does not build itself (numpy, pandas, requests, sqlite3, Pillow, ...) still builds natively: each library call runs in CPython, loaded into the native program, so results are CPython's own, and the rest of the program stays native and fast. Brevi's own modules (math, re, json, datetime, ...) stay native, and what they lack (`math.gcd`, `d.strftime`) is taken from CPython.

Limits today:

- Under `--sandbox` (and in the MCP tools, which sandbox by default) a program that imports such a library must declare `allow native` (from v0.24.1).
- The built program needs `python3` with its shared library (`libpython`) on the machine where it runs, unless built with `--standalone`, which carries CPython and the imported packages in the binary (Linux only).
- Library calls are part of the **typed** build only. A library program the typed build rejects falls back to CPython, because the dynamic build does not bridge to CPython.
- Not on Windows: library programs run with CPython there.
- A `with` block on a library object does not pass exception details to its `__exit__`, and named functions cannot yet be passed as callbacks to a library (lambdas can).

## Integers

Python integers have no size limit, and neither do Brevi's. In typed builds a value starts as a 64-bit integer and becomes a big integer as soon as it could pass 64 bits, through returns, closures, `nonlocal` and `global` and the `//` and `%` operators. Integers never silently wrap around, as they would in a release-mode Rust program. Powers of literals are worked out at compile time, so `3 ** 50 % 1000000007` gives Python's answer. Dynamic builds use big integers with CPython's semantics throughout, including bit operations on numbers of any size.

## Errors and run-time behaviour

A Python file keeps Python's run-time behaviour: a mistake Python only finds when a line runs raises at that point, with the same exception type. Brevi does not report it earlier, because that would change what the program does. Run-time error messages are shorter than CPython's tracebacks; error text on stderr is not part of the "same output" promise, but stdout and the exit status are.

## Platforms

| | Linux | macOS | Windows |
| --- | --- | --- | --- |
| Typed and dynamic builds | yes | yes | yes |
| Library calls through CPython | yes | yes | not yet (falls back to CPython) |
| `--standalone` | yes | not yet | not yet |
| GUI (tkinter) programs, headless and `--window` | yes | yes | yes |

## Known gaps

These are tracked on the [roadmap](/docs/roadmap):

- **Library programs the typed build rejects** fall back to CPython instead of building natively.
- **Dynamic builds are slower** than typed ones; widening what the typed build accepts is ongoing work.
- **`async`, `match` and `eval`** are not built natively.
- **Cold builds** take about a second for a small program (a few seconds with `--release`), which matters for agents that build many one-off programs; repeat builds are cached.

# What works, and limits
<!-- description: Which Python Brevi builds natively, how libraries work, typed versus dynamic builds, big integers, and the known gaps. -->

Brevi's promise for a Python file is simple: **the native program prints exactly what CPython prints, or Brevi refuses to build it.** This page says how much Python that covers today and where the edges are.

## In numbers

| Check | Result |
| --- | --- |
| Saved agent programs (591, from benchmark runs) built unchanged | 591 build, 591 match CPython on every test |
| Large programs (18; chess, a text adventure, an interpreter; 200 to 750 lines over 3 to 4 files) | 18 build, 18 match |
| Behaviour traps (13: big ints, `//` and `%` signs, float printing, dict order, unicode, aliasing, deep recursion, ...) | 13 match |
| Release test (24 tasks, Sonnet and Haiku writing fresh Python) | 92 to 98% of final programs native, the rest via the CPython fallback (the two that needed it in run w156a, both `json_query`, now build natively on main) |
| Random generated programs (bug hunt, 2 Nov 2026) | 400 of 400 match |

"Match" means stdout byte for byte and the same exit status. See [Benchmarks](/docs/benchmarks) for the method.

## Typed and dynamic builds

Brevi has two native ways to build a Python file, and picks between them on its own.

**Typed build**, the fast path. Brevi infers a fixed type for every value across the whole program and writes plain Rust. Most agent-written programs build this way. Python behaviour that shows in output is reproduced: unlimited integers, shared mutable objects (`r = rows[k]; r[0] -= 1` changes `rows`), dict printing and order, Python's float formatting and `round`, exception messages.

**Dynamic build**, the exact path. When a value changes type, a list mixes types, or the typed build would print something different from CPython (iterating a set, for example, where CPython's hash order shows), the whole program is built over a dynamic value type with CPython's own semantics: hashing and set order, formatting, exceptions, classes and method lookup. Still native, with no CPython involved, but slower than typed: typically somewhat faster than CPython rather than many times faster.

You do not choose; Brevi tries typed first. A file that does not build typed prints one [`{"path":...}` line](/docs/usage#which-path-ran) naming the function that kept it off the fast path; `brevi check --explain x.py` lists every reason.

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

### What runs native

Standard-library modules are built by Brevi itself, with no CPython. The typed build is the fast path; what it lacks runs in the dynamic build, which is still native code. A program never needs CPython for the modules below (from v0.24.3):

| Module | Typed build (fastest) | Dynamic build (native) |
|---|---|---|
| builtins and `str` | most, including `bin`, `hex`, `oct`, `str.startswith`/`endswith` with a start index | a tuple of prefixes, an end index |
| `math` | `sqrt`, `floor`, `ceil`, `isqrt`, `gcd`, `lcm`, `comb`, `perm`, `factorial`, `isclose`, `log` (with a base too), `log2`, `log10`, `log1p`, `exp`, `expm1`, trig and hyperbolic functions, `atan2`, `hypot`, `fabs`, `trunc`, `degrees`, `radians`, `copysign`, `fmod`, `isnan`, `isinf`, `isfinite`, `pi`, `e`, `tau`, `inf`, `nan`; domain errors are `ValueError`s an `except` catches | the rest (`fsum`, `prod`, `dist`, ...) |
| `time` | `time`, `perf_counter`, `monotonic` | `sleep`, `process_time` |
| `re` | `match`, `search`, `fullmatch`, `findall` (no group or one), `finditer`, `split`, `sub`, `compile`, flags `I`, `S`, `M`, `A` | `findall` with two or more groups, `sub` with a function, groups tested against None, `groupdict` |
| `json` | `loads`, `load`, `dumps` | `dumps(indent=..., sort_keys=...)` on mixed values, `JSONDecoder().raw_decode` and `decode`, `json.decoder.scanstring` |
| `collections` | `Counter`, `defaultdict`, `deque` (`popleft`, `appendleft`), `OrderedDict` | `namedtuple`, `deque(maxlen=...)`, `rotate`, a defaultdict read whose missing key matters |
| `itertools` | `groupby` | `chain`, `product`, `permutations`, `combinations`, `accumulate`, `islice`, `zip_longest`, `pairwise`, `count`, `cycle` and the rest |
| `functools` | `lru_cache`, `cache` | `reduce`, `partial`, `cmp_to_key`, `total_ordering` |
| `dataclasses` | `@dataclass` (defaults, `field(default_factory=...)`, `order`, `frozen`) | `asdict`, `astuple`, `replace` |
| `heapq`, `bisect`, `string`, `sys`, `typing`, `decimal`, `datetime` (`date`) | most functions | the rest, `datetime.datetime`, `strptime`, `strftime` |
| `os`, `os.path`, `pathlib` | | path text: `os.path.join`, `basename`, `dirname`, `split`, `splitext`, `normpath`, `os.sep`, `pathlib.PurePath` (from v0.24.4 outside the sandbox too); files, processes and the environment run in CPython |
| `random` | | `seed` (int), `random`, `randint`, `randrange`, `choice`, `choices`, `shuffle`, `sample`, `uniform`, `gauss`, `getrandbits`, `Random`: CPython's generator, so seeded draws match |
| `textwrap` | | `wrap`, `fill`, `shorten`, `dedent`, `indent`, `TextWrapper` (also under `--sandbox`) |
| bytes, `hashlib`, `base64`, `binascii` | | `b"..."` literals, `bytes(...)`, `str.encode` and `bytes.decode` (UTF-8, ASCII, Latin-1), indexing, slicing, `in`, `+`, `split`, `strip`, `join`, `hex`, `fromhex` and the other common methods; `hashlib.md5`, `sha1`, `sha224`, `sha256`, `sha384`, `sha512` and `new` (`update`, `digest`, `hexdigest`, `copy`); `base64` b64/urlsafe/b16 encode and decode, with CPython 3.13's padding rules and errors; `binascii.hexlify`, `unhexlify`, `Error` |
| `argparse` | | all of it but `FileType` and `fromfile_prefix_chars` (also under `--sandbox`): CPython 3.13's own argparse, so usage lines, help text and error messages match it |
| `io` | | `StringIO` (also under `--sandbox`); `BytesIO` and the rest run in CPython |
| `enum` | | `Enum` (members, aliases, `auto()`, methods and properties, `__init__` on tuple values, `C(value)`, `C[name]`, iteration, `len`, `in`, `__members__`), `class C(str, Enum)` and `(int, Enum)`, `@unique`; `IntEnum`, `StrEnum` and `Flag` still run in CPython |
| `statistics`, `copy`, `operator`, `csv`, `html`, `fractions`, `threading`, `abc` | parts (`operator.lt` and friends, `csv.reader`, `csv.DictReader`, `csv.writer(sys.stdout)`, `Fraction`) | the rest |

Run in CPython for now (`brevi run` still works; under `--sandbox` the caller grants them with `--allow-py`): `random` beyond the common functions (`betavariate`, a str seed, ...), `textwrap` with `fix_sentence_endings`, `argparse.FileType`, `io` beyond `StringIO`, `os` beyond path text, `enum.IntEnum`, `StrEnum` and `Flag`, `bytearray`, binary files and `sys.stdout.buffer`, and `hashlib` beyond MD5 and the SHA-1/SHA-2 family. These are next on the [roadmap](/docs/roadmap).

Not built natively yet (`brevi run` falls back to CPython; `--native` refuses):

- `async` / `await` and `asyncio`
- The `match` statement
- `eval`, `exec` and other run-time code generation
- Modules outside the list above in a program that also needs the dynamic build (see below)

## Libraries (numpy, pandas, ...)

A Python file that imports a module Brevi does not build itself (numpy, pandas, requests, sqlite3, Pillow, ...) still builds natively: each library call runs in CPython, loaded into the native program, so results are CPython's own, and the rest of the program stays native and fast. Brevi's own modules (math, re, json, datetime, ...) stay native: what the typed build lacks of them goes to the dynamic build, never to CPython (from v0.24.3; see [What runs native](#what-runs-native)).

Limits today:

- Under `--sandbox` (and in the MCP tools, which sandbox by default) a program that imports such a library is refused unless the caller grants it with `--allow-py requests` (MCP: `allow_py`) or `--allow native`; an `# allow native` line in the file grants nothing there (from v0.24.6; from v0.24.1 to v0.24.5 the line was enough). Path functions of `os` and `pathlib` and the common `random` functions work without it (from v0.24.2; see [Sandbox](/docs/usage#sandbox)).
- The built program needs `python3` with its shared library (`libpython`) on the machine where it runs, unless built with `--standalone`, which carries CPython and the imported packages in the binary (Linux only).
- A library program the typed build rejects goes to the dynamic build, which calls the library through CPython the same way and keeps the rest native (from v0.24.6). Classes built on a library's classes (`class Post(BaseModel)`) run in CPython as a whole. Such a class falls back to CPython for the whole file if it sits inside a function, uses a name of the program that is not an import, or is subclassed in another file. The path line lists what CPython runs (`"py":[...]`).
- Not on Windows: library programs run with CPython there.
- In typed builds, a `with` block on a library object does not pass exception details to its `__exit__`, and named functions cannot yet be passed as callbacks to a library (lambdas can). Dynamic builds do both.

## Integers

Python integers have no size limit, and neither do Brevi's. In typed builds a value starts as a 64-bit integer and becomes a big integer as soon as it could pass 64 bits, through returns, closures, `nonlocal` and `global` and the `//` and `%` operators. Integers never silently wrap around, as they would in a release-mode Rust program. Powers of literals are worked out at compile time, so `3 ** 50 % 1000000007` gives Python's answer. Dynamic builds use big integers with CPython's semantics throughout, including bit operations on numbers of any size.

## Errors and run-time behaviour

A Python file keeps Python's run-time behaviour: a mistake Python only finds when a line runs (an index out of range, a bad `int()` argument) raises at that point, with the same exception type. The exception is a sure mistake, a name that is never defined, a method no Python type has, or text added to a number: `brevi check` and `brevi build` report that before the program runs, with a fix, and `brevi run` names it in its fallback note. Code that may never run (under `sys.version_info`, `TYPE_CHECKING` or `if False`, or in an `except` block) is left alone. Run-time error messages are shorter than CPython's tracebacks; error text on stderr is not part of the "same output" promise, but stdout and the exit status are.

## Platforms

| | Linux | macOS | Windows |
| --- | --- | --- | --- |
| Typed and dynamic builds | yes | yes | yes |
| Library calls through CPython | yes | yes | not yet (falls back to CPython) |
| `--standalone` | yes | not yet | not yet |
| GUI (tkinter) programs, headless and `--window` | yes | yes | yes |

## Known gaps

These are tracked on the [roadmap](/docs/roadmap):

- **Classes built on a library's classes** (`class Post(BaseModel)`) run in CPython as a whole, and in some layouts send the whole file to CPython.
- **Dynamic builds are slower** than typed ones; widening what the typed build accepts is ongoing work.
- **`async`, `match` and `eval`** are not built natively.
- **Cold builds** take about 0.15 to 0.3 seconds for a small program (a few seconds with `--release`), plus 5 to 15 seconds once for Brevi's runtime on the very first build. That still matters for agents that build many one-off programs; repeat builds are cached.

# Brevi libraries (for models)

## Libraries
`use "geom.bv"` brings in another Brevi file's `fn`, `type`, `enum` and `test` items (path relative to this file; its errors add `"file"`).
For what Brevi lacks, write a function in Rust (any crate) or Python. Parameters and return type are Brevi types; call it like any function.
```
use sha2 "0.10"                    # a crate: use NAME "VERSION" "FEATURES" (built once, then cached)
rust fn sha256(s: str) -> str:     # Rust body: str is String, [T] Vec<T>, {K: V} a map like HashMap, mut x is &mut
  use sha2::{Digest, Sha256};
  format!("{:x}", Sha256::digest(s.as_bytes()))
rust fn hex(s: str) -> int !:      # with !, return Ok(v); ? works on any error; Err("msg".into()) fails
  Ok(i64::from_str_radix(&s, 16)?)
py fn mean(xs: [f64]) -> f64:      # Python body; values cross as JSON (a type is a dict)
  import statistics
  return statistics.mean(xs)
py fn sq(x: int) -> int = x * x    # one-line form, also for rust fn
```
A `py fn` can fail: a Python exception becomes its error. Top-level `rust:` and `py:` blocks hold shared Rust items or Python code. Give crate versions you know: without one cargo takes the newest.

## Python libraries
In Python files (`.py`, or a `.bv` written as Python), `import numpy as np`, `pandas`, `sqlite3`, `subprocess`, `pathlib`, `urllib.parse` or any other installed module runs in CPython inside the built program: its values, methods, operators (`a @ b`), iteration and exceptions (`except sqlite3.Error`) behave as in Python. Brevi's own modules (math, re, json, datetime, ...) stay native and fast, and so does most numpy code: arrays, broadcasting, indexing, reductions and printing build with Brevi's own numpy, matching numpy's output byte for byte (anything outside it gets CPython's numpy; see [numpy](site/docs/compatibility.md#numpy)), and so does `tiktoken`, with the OpenAI vocabularies built into the binary ([tiktoken](site/docs/compatibility.md#tiktoken)), and Hugging Face `tokenizers`, with each `tokenizer.json` built in ([tokenizers](site/docs/compatibility.md#tokenizers)); what they lack (`math.gcd`, `d.strftime(...)`) is taken from CPython. A list passed to a library sees its changes (`random.shuffle(xs)`); a lambda can be passed as a callback (`key=lambda r: r[1]`). The built program needs python3 with its shared library (libpython), unless it is built with `brevi build x.py --standalone`: the binary then carries that CPython, the standard library and the installed packages the program imports (with what they require), and runs on a Linux machine with no Python at all. It unpacks them once into `~/.cache/brevi/py` (or `$BREVI_PY_CACHE`) on first run, about 1 to 3 seconds; later runs start at normal speed. Expect 13 MB for a standard-library program and 30 to 45 MB with numpy or pandas. Results are CPython's own, as without `--standalone`. Linux only for now.

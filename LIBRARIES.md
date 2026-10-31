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

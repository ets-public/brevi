# Axon language guide (for models)

Axon compiles to Rust. Write one statement per line, no semicolons. Types are required only on function parameters and returns.
A file can be just statements (script mode), or define `fn main()`.

```
let nums = stdin().ints()?
print("total {nums.sum()}, mean {(nums.sum().float() / nums.len().float()).fmt(2)}")
```

## Types
`int` (64-bit), `f64`, `bool`, `str`, `[T]` list, `{K: V}` map (sorted by key), `(A, B)` tuple.
No references or lifetimes: values act as copies. A `mut` parameter edits the caller's variable (call it as `f(x)`).
Tuples, lists and strings compare and sort element by element: `pairs.sort_by(p => (-p.1, p.0))`. `let (a, b) = t` destructures.

```
type Point { x: f64, y: f64 }
enum Shape { Circle(r: f64), Rect(w: f64, h: f64), Empty }
let p = Point(x: 1.0, y: 2.0)      // or Point(1.0, 2.0)
let s = Rect(w: 2.0, h: 3.0)
```

## Functions
```
fn area(s: Shape) -> f64 = match s {
  Circle(r) => 3.14159 * r * r
  Rect(w, h) => w * h
  Empty => 0.0
}
fn load(path: str) -> [str] {
  let text = read(path)?               // ? passes the error up, so load can fail
  if text.is_empty() { fail("empty file") }
  text.lines()                         // last line is the return value
}
fn bump(mut xs: [int]) { for x in xs { x += 1 } }   // edits caller's list
fn main() ! { ... }
```
A function that uses `?` or `fail` can fail, and every call to it needs `?` (marking it `!` is optional). `?` also works inside lambdas: `lines().map(l => l.int()?)`.

## Statements
`let x = 1` fixed, `var x = 1` changeable, `x = 2`, `x += 1` (also `-=` `*=` `/=`), `return v`, `break`, `continue`, `assert cond`.
`if c { } else if d { } else { }`, `for x in xs { }`, `for (k, v) in m.pairs() { }`, `for i in 0..n { }`, `while c { }`.
`if` and `match` are values: `let y = if a > b { a } else { b }`.
Match patterns: literals, `_`, names, `Variant(a, b)`, `(a, b)`.

## Operators
`+ - * / %`, `== != < > <= >=`, `and`, `or`, `!`, `a ~= b` (float equal within 1e-6).
`+` joins strings and lists. int and f64 never mix: convert with `.float()` or `.int()`.
Strings interpolate: `"{name} is {age}"` (a `{` not followed by a letter or `(` is a plain brace; `{{` is always a plain brace). Inside `{}` use names or simple expressions without quotes.
Escapes: `\n \t \r \0 \" \\`.

## Collections
`xs[0]`, `xs[-1]` (last), `m[k]` (missing key stops the program), `m[k] = v`, `m[k] += 1` and `m[k].push(x)` (missing key starts at 0, "" or []).
Empty typed values: `var m: {str: int} = {}`, `var xs: [str] = []`.
Lambdas: `x => x * 2`, `(a, b) => a + b`.

## Methods
- str: `len words lines chars split(sep) slice(a, b) find(s) lower upper trim contains starts_with ends_with replace(a, b) repeat(n) is_empty is_digit is_alpha is_space`, and `int() float() ints() big()` which can fail, so use `?`. `ints()` = all whitespace-separated ints
- list: `len is_empty push(x) pop insert(i, x) remove(i) first last slice(a, b) map(f) filter(f) any(f) all(f) count(f) fold(init, f) sort sort_by(f) rev take(n) skip(n) enumerate zip(ys) sum min max contains(x) index_of(x) join(sep)`
- map: `len is_empty has(k) get_or(k, default) remove(k) keys values pairs`
- int and f64: `abs float int fmt(decimals)` (`2.5.fmt(2)` is "2.50"); f64 also `sqrt round floor ceil`; int also `big`
`slice` takes Python-style bounds (negatives count from the end). `find` and `index_of` return -1 when absent. `"7".digit()` is 7 (-1 if not a digit). `len` counts characters. `pop` returns the removed item. `and`/`or` short-circuit.
Handle a failure in place with `.or(default)`: `s.int().or(0)`, `load(p).or([])`.

Most methods return a new value: write `xs = xs.sort()`. Only `push pop insert remove` change the list in place.

## Built-in functions
`print(x)`, `str(x)`, `read(path)?`, `write(path, text)?`, `args()`, `input()` (one line), `stdin()` (all text), `lines()` (stdin as a list of lines), `fail(msg)`, `abs(x)`, `min(a, b)`, `max(a, b)`.

## Data formats
- `json(text)?` gives a `Json`: match on `Null`, `Bool(b)`, `Num(n)` (f64), `Str(s)`, `Arr(xs)`, `Obj(m)` ({str: Json}), or index `j["key"][0]` (missing gives `Null`). `str(j)` writes JSON.
- `csv(text)` gives `[[str]]`, handling quoted fields and `""`.
- `date("2024-02-29")?` gives a `Date` with `days` (since 1970-01-01), `add_days(n)`, `year month day weekday` (1 = Monday); `str(d)` is `YYYY-MM-DD`.
- `big` is an exact integer of any size: `"123...".big()?`, `n.big()`, with `+ - *` and comparisons.

## Tests
```
test "area of unit circle" { assert area(Circle(1.0)) ~= 3.14159 }
```

## Commands
`axon run f.ax`, `axon test f.ax`, `axon check f.ax`, `axon build f.ax -o app [--release]`, `axon emit f.ax` (shows the Rust).
Errors are one JSON line each: `{"e":kind,"at":"file:line:col","msg":...,"fix":...}`. Apply the fix and rerun. Runtime errors print `{"e":"runtime","line":N,"msg":...}`.

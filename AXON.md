# Axon language guide (for models)

Axon compiles to Rust. Write one statement per line, no semicolons. Comments start with `//`. Types are required only on function parameters and returns.
A file can be just statements (script mode), or define `fn main()`. In script mode, functions can read and change top-level variables (`pos = 0`; give a type when the value is not a literal: `toks: [str] = []`).

```
nums = stdin().ints()
print("total {nums.sum()}, mean {(nums.sum().float() / nums.len().float()).fmt(2)}")
```

## Types
`int` (64-bit), `f64`, `bool`, `str`, `[T]` list, `{K: V}` map (sorted by key), `(A, B)` tuple, `dec` exact decimal, `big` exact integer, `Date`, `Json`, `heap[T]` min-heap.
No references or lifetimes: values act as copies. Local variables never need a type. A `mut` parameter edits the caller's variable (call it as `f(x)`).
Tuples, lists and strings compare and sort element by element: `pairs.sort_by(p => (-p.1, p.0))`. `let (a, b) = t` destructures; `p.0` is a field.

```
type Point { x: f64, y: f64, label: str = "" }   // a field with = has a default
enum Shape { Circle(r: f64), Rect(w: f64, h: f64), Empty }
p = Point(x: 1.0, y: 2.0)      // or Point(1.0, 2.0), or Point(x, y) when variables x and y exist
s = Rect(w: 2.0, h: 3.0)
```

## Functions
```
fn area(s: Shape) -> f64 = match s {
  Circle(r) => 3.14159 * r * r
  Rect(w, h) => w * h
  Empty => 0.0
}
fn load(path: str) -> [str] {
  text = read(path)                    // read can fail; its error passes up, so load can fail
  if text.is_empty() { fail("empty file") }
  text.lines()                         // last line is the return value
}
fn bump(mut xs: [int]) { for x in xs { x += 1 } }   // edits caller's list
fn main() ! { ... }
```
Errors pass up automatically. `fail(msg)`, `read`, and conversions like `s.int()` can fail; a function that uses any of them can fail, and so can its callers. No `?` is needed (writing it is allowed).
Handle a failure where it happens with `.or(default)` or `match f(x) { Ok(v) => ..., Err(msg) => ... }`. An uncaught failure stops the program with a JSON error.

## Statements
`x = 1` creates a variable or changes it (`let x = 1` makes one that cannot change, `x: [str] = []` gives a type), `(a, b) = pair` unpacks, `x += 1` (also `-=` `*=` `/=`), `return v`, `break`, `continue`, `assert cond`.
`if c { } else if d { } else { }`, `for x in xs { }`, `for (k, v) in m.pairs() { }`, `for i in 0..n { }`, `while c { }`.
`if` and `match` are values: `y = if a > b { a } else { b }`. A variable first set inside a block ends with the block, so set it before the block if you need it after.
Match patterns: literals, `_`, names, `Variant(a, b)`, `(a, b)`.

## Operators
`+ - * / %`, `== != < > <= >=`, `and` `or` `!` (also `&&` `||` `not`), `a ~= b` (float equal within 1e-6).
`+` joins strings and lists. int and f64 never mix: convert with `.float()` or `.int()`.
Strings interpolate: `"{name} is {age}"`, `"{-x} {xs.len()} {p.0}"` (`{` followed by a letter, `(`, `-` or `!` starts an expression; any other `{` and `{{` are plain braces). No quotes inside `{}`.
Escapes: `\n \t \r \0 \" \\`.

## Collections
`xs[0]`, `xs[-1]` (last), `m[k]` (missing key stops the program), `m[k] = v`, `m[k] += 1` and `m[k].push(x)` (missing key starts at 0, "" or []).
Empty values: `xs = []`, `m = {}` (add a type like `m: {str: int} = {}` only if the compiler asks).
Lambdas: `x => x * 2`, `(a, b) => a + b`. In map/filter/any/all/count/sort_by, `(a, b) => b - a` unpacks a tuple item.

## Methods
- str: `len words lines chars split(sep) slice(a, b) find(s) lower upper trim contains starts_with ends_with replace(a, b) repeat(n) is_empty is_digit is_alpha is_space`, and `int() float() ints() big() dec()` which can fail. `ints()` = all whitespace-separated ints
- list: `len is_empty push(x) pop insert(i, x) remove(i) first last slice(a, b) map(f) filter(f) any(f) all(f) count(f) fold(init, f) sort sort_by(f) rev take(n) skip(n) enumerate zip(ys) sum min max contains(x) index_of(x) join(sep)`
- map: `len is_empty has(k) get_or(k, default) remove(k) keys values pairs`
- int and f64: `abs float int fmt(decimals)` (`2.5.fmt(2)` is "2.50"); f64 also `sqrt round floor ceil`; int also `big`
`slice` takes Python-style bounds (negatives count from the end). `find` and `index_of` return -1 when absent. `"7".digit()` is 7 (-1 if not a digit). `len` counts characters. `pop` and `remove(i)` return the removed item. `words` splits on any run of whitespace. `and`/`or` short-circuit.
`s.int().or(0)`, `load(p).or([])` give a default instead of failing.

Most methods return a new value: write `xs = xs.sort()`. Only `push pop insert remove` change the list in place.

## Built-in functions
`print(x)`, `str(x)`, `read(path)`, `write(path, text)`, `args()`, `input()` (one line), `stdin()` (all text), `lines()` (stdin as a list of lines), `fail(msg)`, `abs(x)`, `min(a, b)`, `max(a, b)`.

## Data formats
- `json(text)` gives a `Json`: match on `Null`, `Bool(b)`, `Num(n)` (f64), `Str(s)`, `Arr(xs)`, `Obj(m)` ({str: Json}), or index `j["key"][0]` (missing gives `Null`). `str(j)` writes JSON.
- `csv(text)` gives `[[str]]`, handling quoted fields and `""`.
- `date("2024-02-29")` gives a `Date` with `days` (since 1970-01-01), `add_days(n)`, `year month day weekday` (1 = Monday); `str(d)` is `YYYY-MM-DD`.
- `big` is an exact integer of any size: `"123...".big()`, `n.big()`, with `+ - *` and comparisons.
- `dec` is an exact decimal (9 places) for money: `"19.99".dec()`, `n.dec()`, `+ - * /`, unary `-`, comparisons, `sum`, `fmt(2)` (rounds half to even), `float int round`.
- `heap()` is a min-heap: `h.push(x)`, `h.pop()` (smallest; tuples compare in order), `first len is_empty`.

## Tests
```
test "area of unit circle" { assert area(Circle(1.0)) ~= 3.14159 }
```

## Commands
`axon run f.ax`, `axon test f.ax`, `axon check f.ax`, `axon build f.ax -o app [--release]`, `axon emit f.ax` (shows the Rust).
Errors are one JSON line each: `{"e":kind,"at":"file:line:col","msg":...,"fix":...}`. Apply the fix and rerun. Runtime errors print `{"e":"runtime","line":N,"msg":...}`.

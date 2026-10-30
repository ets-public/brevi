# Axon language guide (for models)

Axon compiles to Rust. Write one statement per line, no semicolons. Types are required only on function parameters and returns.

## Types
`int` (64-bit), `f64`, `bool`, `str`, `[T]` list, `{K: V}` map (sorted by key), `(A, B)` tuple.
No references or lifetimes: values act as copies. A `mut` parameter edits the caller's variable.

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
fn load(path: str) -> [str] ! {      // ! = can fail
  let text = read(path)?               // ? passes the error up
  if text.is_empty() { fail("empty file") }
  text.lines()                         // last line is the return value
}
fn bump(mut xs: [int]) { for x in xs { x += 1 } }   // edits caller's list
fn main() ! { ... }
```
Calling a function marked `!` needs `?`, and only works inside another `!` function.

## Statements
`let x = 1` fixed, `var x = 1` changeable, `x = 2`, `x += 1` (also `-=` `*=` `/=`), `return v`, `break`, `continue`, `assert cond`.
`if c { } else if d { } else { }`, `for x in xs { }`, `for (k, v) in m.pairs() { }`, `for i in 0..n { }`, `while c { }`.
`if` and `match` are values: `let y = if a > b { a } else { b }`.
Match patterns: literals, `_`, names, `Variant(a, b)`, `(a, b)`.

## Operators
`+ - * / %`, `== != < > <= >=`, `and`, `or`, `!`, `a ~= b` (float equal within 1e-6).
`+` joins strings and lists. int and f64 never mix: convert with `.float()` or `.int()`.
Strings interpolate: `"{name} is {age}"`. Use `{{` for a literal brace. Inside `{}` use names or simple expressions without quotes.

## Collections
`xs[0]`, `xs[-1]` (last), `m[k]` (missing key stops the program), `m[k] = v`, `m[k] += 1` (missing key starts at 0, "" or []).
Lambdas: `x => x * 2`, `(a, b) => a + b`.

## Methods
- str: `len words lines chars split(sep) lower upper trim contains starts_with ends_with replace(a, b) repeat(n) is_empty`, `int()` and `float()` (both fail, use `?`)
- list: `len is_empty push(x) pop insert(i, x) remove(i) first last map(f) filter(f) any(f) all(f) count(f) fold(init, f) sort sort_by(f) rev take(n) skip(n) enumerate zip(ys) sum min max contains(x) index_of(x) join(sep)`
- map: `len is_empty has(k) get_or(k, default) remove(k) keys values pairs`
- int and f64: `abs float int`; f64 also `sqrt round floor ceil`

List methods return new lists; only `push pop insert remove` change the list.

## Built-in functions
`print(x)`, `str(x)`, `read(path)!`, `write(path, text)!`, `args()`, `input()` (one line), `stdin()` (all), `fail(msg)`.

## Tests
```
test "area of unit circle" { assert area(Circle(1.0)) ~= 3.14159 }
```

## Commands
`axon run f.ax`, `axon test f.ax`, `axon check f.ax`, `axon build f.ax -o app [--release]`, `axon emit f.ax` (shows the Rust).
Errors are one JSON line each: `{"e":kind,"at":"file:line:col","msg":...,"fix":...}`. Apply the fix and rerun.

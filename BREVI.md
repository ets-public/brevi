# Brevi language guide (for models)

Brevi compiles to Rust. Write it like Python: one statement per line, indented blocks after a line ending in `:` (a short block can follow the `:`), `#` comments. Only function parameters need types.
A file can be just statements or define `fn main()`. Top-level names are shared by every function, like Python globals without `global`; a function whose body starts by setting a name (`i = 0`) has its own local.
`use "util.bv"` brings in another file's definitions. This guide is the core: `brevi guide WORD` prints every line about a name, and `brevi guide Strings`, `Json`, `Regex`, `Dates`, `Numbers`, `Python` or `Libraries` a section of details.

```
nums = stdin().ints()
print("total {nums.sum()}, mean {(nums.sum().float() / nums.len().float()).fmt(2)}")
```

## Types
`int` (64-bit), `f64`, `bool`, `str`, `[T]` list, `{K: V}` map (sorted by key), `(A, B)` tuple, `dec` exact decimal, `big` exact integer, `Date`, `Json`, `heap[T]` min-heap.
Values act as copies. A `mut` parameter edits the caller's variable; other parameters are local copies.
Tuples, lists and strings compare element by element: `pairs.sort_by(p => (-p.1, p.0))`. `p.0` is a tuple field (`p[0]` also works).

```
type Point:
  x: f64
  y: f64
  label: str = ""              # default
enum Shape:
  Circle(r: f64)
  Rect(w: f64, h: f64)
  Empty
p = Point(x: 1.0, y: 2.0)      # or Point(1.0, 2.0)
s = Rect(w: 2.0, h: 3.0)
```

## Functions
```
fn area(s: Shape) -> f64 = match s:
  Circle(r) => 3.14159 * r * r
  Rect(w, h) => w * h
  Empty => 0.0
fn load(path: str) -> [str]:
  text = read(path)                    # can fail; the error passes up
  if text.is_empty(): fail("empty file")
  text.lines()                         # last line is returned
fn bump(mut xs: [int]):
  for x in xs: x += 1                  # edits caller's list
```
Errors pass up automatically, no `?`: a function using `fail(msg)` (like Python's raise), `read` or `s.int()` can fail, and so can its callers. Handle a failure with `.or(default)` (`s.int().or(0)`) or `match f(x):` with arms `Ok(v) => ...` and `Err(msg) => ...`.

## Statements
`x = 1` creates or changes a variable (`x: [str] = []` gives a type), `a, b = b, a + b`, `x += 1`, `return v`, `break`, `continue`, `assert cond`.
`if c:` / `elif d:` / `else:` with Python truth (`if xs:`, `if not s:`), `for x in xs:`, `for k, v in m.pairs():`, `for i in 0..n:`, `while c:`.
`if` and `match` are values: `y = if a > b: a else: b`. A variable set in every branch of an `if`/`else` or `match` can be used after it; one first set inside a loop ends with the loop, so set it before the loop.
Match arms are one expression or statement, or an indented block after `=>`. Patterns: literals, `_`, names, `Variant(a, b)`, `(a, b)`.

## Operators
`+ - * / // %` (int `/` and `%` round toward zero, `//` rounds down), `== != < > <= >=`, `x in xs` and `x not in xs` (list item, map key or substring), `and` `or` `not`, `a ~= b` (float equal within 1e-6). `+` joins strings and lists.
int and f64 never mix: convert with `.float()` or `.int()` (a whole-number literal next to an f64 is an f64: `x * 2`).
Strings interpolate `"{name} is {age}, {xs.len()} items"` and take Python format specs: `"{x:.2f} {n:03d} {s:<10}"`. In double quotes `{{` is ONE brace. For JSON or template text use `'single quotes'`: no `\"` needed and `{` stays plain unless a name follows (`'{"id":{i}}'`, `'{{#each '`). Details: `brevi guide Strings`.

## Collections
`xs[-1]`, slices `xs[a:b]` `s[1:]`, `xs[i] = v`, `m[k] += 1`, `m[k].push(x)`, `books[id].shelf -= 1` change the stored item (`b = books[id]` is a copy). A missing map key reads as the type's zero; `m.has(k)` tells.
Empty values `xs = []`, `m = {}`, `h = heap()` need no type.
Comprehensions, `len sum min max sorted any all` and tuple map keys (`dist[(x, y)]`) work as in Python.
Lambdas: `x => x * 2`, `(a, b) => a + b` (also unpacks a tuple item).

## Methods
Every call has parentheses: `xs.first()`.
- str: `len words lines chars split(sep) slice(a, b) find(s) lower upper trim contains starts_with ends_with replace(a, b) repeat(n) is_empty is_digit is_alpha is_space code partition(sep)`, and `int() float() ints() big() dec()`, which can fail
- list: `len is_empty push(x) pop insert(i, x) remove(i) first last slice(a, b) map(f) filter(f) any(f) all(f) count(f) fold(init, f) sort sort_by(f) rev take(n) skip(n) enumerate zip(ys) sum min max contains(x) index_of(x) join(sep) unique repeat(n) step(n)`
- map: `len is_empty has(k) get_or(k, default) remove(k) keys values pairs`
- int and f64: `abs float int fmt(decimals)`; f64 also `sqrt round floor ceil`; int also `big char`
Most methods return a new value (`ys = xs.sort()`); as a statement `xs.sort()` sorts xs, and `push pop insert remove` change it in place. `xs.max()` can fail (`.or(0)`); `find` and `index_of` give -1 when absent; `a..b` excludes b.
Python spellings work too (`range()`, `.append()`, `.items()`, `", ".join(xs)`, `lambda x:`, `sorted(xs, key=f)`): `brevi guide Python`.

## Built-in functions
`print(a, b)`, `str(x)`, `read(path)`, `write(path, text)`, `args()`, `input()` (one line), `stdin()` (all text), `lines()` (stdin lines), `fail(msg)`, `abs(x)`, `min(a, b)`, `max(a, b)`.

## Data formats
- `json(text)` gives a `Json`, read directly: `j["key"][0]`, `j.text() j.int() j.dec() j.float() j.is_null() j.has(k) j.keys() j.len()`, `for x in j["items"]:`, `j["tier"] == "gold"`. `str(j)` writes compact JSON; `str({"id": 1, "name": n})` builds an object in written key order. `brevi guide Json`.
- Regexes in Python syntax: `s.re_full(p)` and `s.re_find(p)` give `[whole, group1, ...]` or `[]`; also `re_all re_list re_sub re_split`. `brevi guide Regex`.
- `csv(text)` gives `[[str]]`; `date("2024-02-29")` gives a `Date` (`brevi guide Dates`); `"19.99".dec()` is exact money and `"123".big()` an integer of any size (`brevi guide Numbers`); `heap()` has `push pop` (smallest first).

## Tests
```
test "area of unit circle":
  assert area(Circle(1.0)) ~= 3.14159
```

## Commands
`brevi run|test|check f.bv`, `brevi build f.bv -o app`, `brevi patch f.bv p.bv` (replaces each `fn`/`type`/`test` by name), `brevi fix f.bv` (applies suggested renames).
Errors are one JSON line each, `{"line":N,"msg":...,"fix":...}`. To fix one, change only the lines it names; do not rewrite the whole file.

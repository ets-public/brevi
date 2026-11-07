# Brevi language guide (for models)

Brevi compiles to Rust. Write it like Python: one statement per line, indented blocks after a line ending in `:` (a short block can follow the `:`), `#` comments. Types are optional: parameters, return types and empty values are worked out from use. Write one only when `brevi check` asks.
A file can be just statements or define `fn main()`. Scope works as in Python, `global x` included. Without any `global` line, functions may also set top-level names directly.
`use "util.bv"` brings in another file's definitions. This guide is the core: `brevi guide WORD` prints every line about a name, and `brevi guide Strings`, `Lists`, `Python` or `Libraries` a section of details.

```
nums = stdin().ints()
print("total {nums.sum()}, mean {(nums.sum().float() / nums.len().float()).fmt(2)}")
```

## Types
`int` (64-bit), `f64`, `bool`, `str`, `[T]` list, `{K: V}` map (sorted by key), `(A, B)` tuple, `dec` exact decimal, `big` exact integer, `Date`, `Json`, `heap[T]` min-heap.
A parameter changed in place (`xs.push(x)`, `xs[i] = v`, `p.n += 1`) changes the caller's value, as in Python. Mark it `mut` only to replace it whole (`mut xs`, then `xs = xs.filter(f)`). Other values act as copies.
Tuples, lists and strings compare element by element: `pairs.sort_by(p => (-p.1, p.0))`. `p.0` is a tuple field (`p[0]` also works).

```
type Point:
  x: f64
  y: f64
  label: str = ""              # default
  fn dist(self) = (self.x * self.x + self.y * self.y).sqrt()   # a method: p.dist()
enum Shape:
  Circle(r: f64)
  Rect(w: f64, h: f64)
  Empty
p = Point(x: 1.0, y: 2.0)      # or Point(1.0, 2.0)
s = Rect(w: 2.0, h: 3.0)
```

## Functions
```
fn area(s) = match s:
  Circle(r) => 3.14159 * r * r
  Rect(w, h) => w * h
  Empty => 0.0
fn load(path):
  text = read(path)                    # can fail; the error passes up
  if text.is_empty(): fail("empty file")
  text.lines()                         # last line is returned
fn add(xs, x):
  xs.push(x)                           # edits caller's list
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
Comprehensions (`[*xs for xs in xss]` joins lists), `len sum min max sorted any all` and tuple map keys (`dist[(x, y)]`) work as in Python.
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
- `json(text)` gives a `Json`. Read it directly, no match needed: `j["key"][0]` (missing gives `Null`), `j.text()` (a string's text, `""` for null, other values as JSON), `j.int()`, `j.dec()`, `j.float()` (0 unless a JSON number, so `"7"` gives 0), `j.is_null()`, `j.has(k)`, `j.keys()`, `j.len()`, `for x in j["items"]:`, `if j["ok"]:` (null, false, 0, "" and [] are false), `j["tier"] == "gold"`, `m[j["id"]]`. To tell kinds apart, match on `Null`, `Bool(b)`, `Num(n)` (f64), `Str(s)`, `Arr(xs)`, `Obj(m)`. `str(j)` writes compact JSON. `x.json()` turns any value into Json. A map literal whose values differ in type is a Json object with keys in written order: `str({"id": 1, "name": n})` gives `{"id":1,"name":"ann"}`.
- Regexes (Python syntax: `\d \w \s . [a-z] [^x] * + ? {2,4} *? ( ) (?: ) | ^ $ \b`): `s.re_full(p)` (whole string) and `s.re_find(p)` (first match) give `[whole, group1, ...]`, or `[]` when there is no match; `s.re_all(p)` gives one such list per match; `s.re_list(p)` gives each match's text (a tokenizer: `s.re_list("\d+|[A-Z]+\d*|\S")`); `s.re_sub(p, "<b>\1</b>")` or `s.re_sub(p, g => g[1].upper())`; `s.re_split(p)`. Write `"\d+"` (unknown escapes keep their backslash).
- `csv(text)` gives `[[str]]`, handling quoted fields and `""`.
- `date("2024-02-29")` gives a `Date` with `days` (since 1970-01-01), `add_days(n)`, `year month day weekday` (1 = Monday); `str(d)` is `YYYY-MM-DD`.
- `big` is an exact integer of any size: `"123...".big()`, `n.big()`, with `+ - *` and comparisons.
- `dec` is an exact decimal (9 places) for money: `"19.99".dec()`, `0.dec()`, `+ - * /`, unary `-`, and ints mix in (`0 - p`, `p * 2`, `t += 1`, `p > 0`), `sum`, `fmt(2)` and `round(2)` (round half to even), `float int round`.
- `heap()` is a min-heap of any comparable values: `h.push(x)`, `h.pop()` (smallest; tuples compare in order), `first len is_empty`.

## Tests
```
test "area of unit circle":
  assert area(Circle(1.0)) ~= 3.14159
```

## Commands
`brevi run|test|check f.bv`, `brevi build f.bv -o app`, `brevi patch f.bv p.bv` (replaces each `fn`/`type`/`test` by name), `brevi fix f.bv` (applies suggested renames).
Errors are one JSON line each, `{"line":N,"msg":...,"fix":...}`. To fix one, change only the lines it names; do not rewrite the whole file.

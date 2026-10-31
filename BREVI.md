# Brevi language guide (for models)

Brevi compiles to Rust. Write one statement per line, no semicolons. Blocks are indented, like Python: a line ending in `:` opens a block of the more-indented lines below it, and a short block can follow the `:` on the same line. Comments start with `#`. Types are required only on function parameters; a return type is optional (`fn f(x: int) = x * 2` works it out).
A file can be just statements (script mode), or define `fn main()`. In script mode, functions can read and change top-level variables that other functions also use (`pos = 0`, `toks = []`). A function whose body starts by setting a name (`i = 0`) has its own local, unless another function reads that name without setting it.
`use "util.bv"` at the top of a file brings in that file's fns, types and enums (a used file holds only definitions, no top-level statements).

```
nums = stdin().ints()
print("total {nums.sum()}, mean {(nums.sum().float() / nums.len().float()).fmt(2)}")
```

## Types
`int` (64-bit), `f64`, `bool`, `str`, `[T]` list, `{K: V}` map (sorted by key), `(A, B)` tuple, `dec` exact decimal, `big` exact integer, `Date`, `Json`, `heap[T]` min-heap.
No references or lifetimes: values act as copies. Local variables never need a type. A `mut` parameter edits the caller's variable (call it as `f(x)`); other parameters are local copies the function may change.
Tuples, lists and strings compare and sort element by element: `pairs.sort_by(p => (-p.1, p.0))`. `let (a, b) = t` destructures; `p.0` is a field (`p[0]` also works when the items share a type).

```
type Point:
  x: f64
  y: f64
  label: str = ""              # a field with = has a default
enum Shape:
  Circle(r: f64)
  Rect(w: f64, h: f64)
  Empty
p = Point(x: 1.0, y: 2.0)      # or Point(1.0, 2.0), or Point(x, y) when variables x and y exist
s = Rect(w: 2.0, h: 3.0)
```

## Functions
```
fn area(s: Shape) -> f64 = match s:
  Circle(r) => 3.14159 * r * r
  Rect(w, h) => w * h
  Empty => 0.0
fn load(path: str) -> [str]:
  text = read(path)                    # read can fail; its error passes up, so load can fail
  if text.is_empty(): fail("empty file")
  text.lines()                         # last line is the return value
fn bump(mut xs: [int]):
  for x in xs: x += 1                  # edits caller's list
fn double(x: int) = x * 2              # return type worked out; write -> T if asked
```
Errors pass up automatically. `fail(msg)`, `read`, and conversions like `s.int()` can fail; a function that uses any of them can fail, and so can its callers. No `?` is needed (writing it is allowed).
Handle a failure where it happens with `.or(default)` or `match f(x):` with arms `Ok(v) => ...` and `Err(msg) => ...`. An uncaught failure stops the program with a JSON error.

## Statements
`x = 1` creates a variable or changes it (`x: [str] = []` gives a type), `a, b = pair` unpacks (`a, b = b, a + b` swaps), `x += 1` (also `-=` `*=` `/=`), `return v`, `break`, `continue`, `assert cond`.
`if c:` / `elif d:` / `else:` (conditions use Python truth: `if xs:`, `if not s:`, `while q:` test for non-empty, and 0 is false), `for x in xs:`, `for k, v in m.pairs():` (any list of tuples unpacks this way), `for i in 0..n:`, `while c:`.
`if` and `match` are values: `y = if a > b: a else: b`. A variable set in every branch of an `if`/`else` or `match` can be used after it; one first set inside a loop ends with the loop, so set it before the loop.
Match arms are one expression or statement (`Num(n) => total += n`) or an indented block on the lines after `=>`. Match patterns: literals, `_`, names, `Variant(a, b)`, `(a, b)`.

## Operators
`+ - * / // %` (int `/` and `%` round toward zero, `//` rounds down), `== != < > <= >=`, `x in xs` and `x not in xs` (list item, map key or substring), `and` `or` `!` (also `&&` `||` `not`), `a ~= b` (float equal within 1e-6).
`+` joins strings and lists. int and f64 never mix: convert with `.float()` or `.int()` (a whole-number literal next to an f64 is an f64: `x * 2`, `area(2, 3.5)`).
Strings interpolate: `"{name} is {age}"`, `"{-x} {xs.len()} {p.0}"` (`{` followed by a letter, `(`, `-`, `!` or `"` starts an expression, and strings may appear inside: `"{m["k"]}"`). Any other `{` is a plain brace; `\{` and `{{` always are, so `"{{"` is ONE brace (write `'{{'` for two); template tags such as `"{{#each "`, `"{{/if}}"` and `"{{else}}"` keep both braces.
Escapes: `\n \t \r \0 \" \\ \{ \}`.
`'single quotes'` need no `\"` and keep `{` plain unless a name or `(` follows (`{{` is two braces), so JSON and template text is easy: `'{"id":{i},"error":"not found"}'`, `t.starts_with('{{#each ')`.

## Collections
`xs[0]`, `xs[-1]` (last), `xs[a:b]` `xs[1:]` `s[:3]` (slices, also on strings), `xs[i] = v`, `m[k] = v`, `m[k] += 1`, `m[k].push(x)`. A missing map key reads as the type's zero (0, 0.0, "", [], dec 0); use `m.has(k)` to tell.
Empty values: `xs = []`, `m = {}`, `h = heap()`. Don't write their types: the compiler works them out from later use (add one like `m: {str: int} = {}` only if it asks).
Comprehensions: `[x * 2 for x in xs if x > 0]`, `[k for k, v in m.pairs() if v > 1]`, `[*xs for xs in xss]` joins lists, and in calls `min(d for d in ds)`. `len(x)`, `sum(xs)`, `min(xs)`, `max(xs)`, `sorted(xs)`, `any(...)`, `all(...)` work as in Python. Map keys can be tuples: `dist[(x, y)]`.
Lambdas: `x => x * 2`, `(a, b) => a + b`. In map/filter/any/all/count/sort_by, `(a, b) => b - a` unpacks a tuple item.

## Methods
Every call has parentheses, even with no arguments: `xs.first()`, `s.trim()`.
- str: `len words lines chars split(sep) slice(a, b) find(s) lower upper trim contains starts_with ends_with replace(a, b) repeat(n) is_empty is_digit is_alpha is_space code` (`"a".code()` is 97), and `int() float() ints() big() dec()` which can fail (`s.int().or(-1)` to test). `ints()` = all whitespace-separated ints
- list: `len is_empty push(x) pop insert(i, x) remove(i) first last slice(a, b) map(f) filter(f) any(f) all(f) count(f) fold(init, f) sort sort_by(f) rev take(n) skip(n) enumerate zip(ys) sum min max contains(x) index_of(x) join(sep) (any items) unique repeat(n) step(n)` (`[0].repeat(n)` is n zeros; `unique` sorts and drops repeats; `(0..10).step(3)` is [0, 3, 6, 9]; `(0..n).rev()` counts down)
- map: `len is_empty has(k) get_or(k, default) remove(k) keys values pairs`
- int and f64: `abs float int fmt(decimals)` (`2.5.fmt(2)` is "2.50"); f64 also `sqrt round floor ceil`, `round(n)` to n places; int also `big char` (`97.char()` is "a")
`min` and `max` fail on an empty list (`xs.max().or(0)`). `a..b` excludes b. `slice` takes Python-style bounds (negatives count from the end); `s.slice(a)` runs to the end and `s.slice(i, i + 1)` is one character. `chars` gives a list of one-character strings. `first`, `last` and `pop` return the item itself and stop the program on an empty list. `find` and `index_of` return -1 when absent. `"7".digit()` is 7 (-1 if not a digit). `len` counts characters. `pop` and `remove(i)` return the removed item. `words` splits on any run of whitespace. `and`/`or` short-circuit.
`s.int().or(0)`, `load(p).or([])` give a default instead of failing.

Most methods return a new value (`ys = xs.sort()`); as a statement, `xs.sort()` sorts xs. `push pop insert remove` change the list in place.
Python spellings work too: `True False`, `a if c else b`, `range(a, b, step)`, `for c in s`, `enumerate(xs) int(s) ord chr reversed`, `append strip split() startswith endswith isdigit items index get(k, d)`, `", ".join(xs)`, `(a, _, b) = s.partition("=")`.

## Built-in functions
`print(x)` (`print(a, b)` puts a space between), `str(x)` (or `x.str()`), `read(path)`, `write(path, text)`, `args()`, `input()` (one line), `stdin()` (all text), `lines()` (stdin as a list of lines), `fail(msg)`, `abs(x)`, `min(a, b)`, `max(a, b)`.

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
`brevi run f.bv`, `brevi test f.bv`, `brevi check f.bv`, `brevi build f.bv -o app [--release]`, `brevi emit f.bv` (shows the Rust). `brevi guide WORD` prints the guide lines about WORD. `brevi patch f.bv p.bv` replaces each `fn`/`type`/`test` in p.bv by name. `brevi guide Libraries` shows how to use other .bv files, Rust crates and Python.
Errors are one JSON line each: `{"line":N,"msg":...,"fix":...}`. Runtime errors print `{"e":"runtime","line":N,"msg":...}`. A small typo of a known name is fixed for you (`{"line":N,"fixed":"totl -> total"}`); `brevi fix f.bv` writes every suggested rename into the file.
To fix an error, change only the lines it names (edit them in place); do not rewrite the whole file.

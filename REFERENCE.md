# Brevi reference (for models)

Details kept out of the core guide, which every session reads. `brevi guide SECTION` prints one section; `brevi guide WORD` prints every line about a name.

## Strings
Strings interpolate: `"{name} is {age}"`, `"{-x} {xs.len()} {p.0}"` (`{` followed by a letter, `(`, `-`, `!` or `"` starts an expression, and strings may appear inside: `"{m["k"]}"`). Any other `{` is a plain brace; `\{` and `{{` always are, so `"{{"` is ONE brace (write `'{{'` for two); template tags such as `"{{#each "`, `"{{/if}}"` and `"{{else}}"` keep both braces.
Python format specs work inside `{}`: `"{x:.2f} {n:03d} {name:<10} {total:>8,}"`.
Escapes: `\n \t \r \0 \" \\ \{ \}`.
`'single quotes'` need no `\"` and keep `{` plain unless a name or `(` follows (`{{` is two braces), so JSON and template text is easy: `'{"id":{i},"error":"not found"}'`, `t.starts_with('{{#each ')`.
`"a".code()` is 97 and `97.char()` is "a". `"7".digit()` is 7 (-1 if not a digit). `len` counts characters; `chars` gives a list of one-character strings; `words` splits on any run of whitespace; `ints()` gives all whitespace-separated ints.
`s.slice(a)` runs to the end and `s.slice(i, i + 1)` is one character; negative bounds count from the end. `s.int().or(-1)` tests a number. `(a, _, b) = s.partition("=")`.

## Lists
`[0].repeat(n)` is n zeros; `unique` sorts and drops repeats; `(0..10).step(3)` is [0, 3, 6, 9]; `(0..n).rev()` counts down. `join(sep)` takes any items.
`xs.sort_by(f, desc)` sorts largest first when `desc` is true, keeping ties in order (as `sorted(xs, key=f, reverse=desc)`).
`first`, `last` and `pop` return the item itself and stop the program on an empty list; `min` and `max` fail on an empty list. `pop` and `remove(i)` return the removed item. In map/filter/any/all/count/sort_by, `(a, b) => b - a` unpacks a tuple item. `and`/`or` short-circuit.
`2.5.fmt(2)` is "2.50"; f64 `round(n)` rounds to n places.

## Type details
`let (a, b) = t` or `(a, b) = t` destructures a tuple. `Point(x, y)` works as a constructor when variables x and y exist. A `mut` parameter is called as `f(x)`. A return type is optional (`fn f(x: int) = x * 2`); write `-> T` if an error asks.
A missing map key reads as the type's zero (0, 0.0, "", [], dec 0). `print(a, b)` puts a space between.

## Python
Python spellings models write by habit are accepted: `True False`, `a if c else b`, `range(a, b, step)`, `for c in s`, `enumerate(xs) int(s) ord chr reversed`, `lambda x: x * 2`, `min(xs, key=f)`, `sorted(xs, key=f, reverse=True)`, `print(x, end="", sep=", ")`, `append strip split() startswith endswith isdigit items index get(k, d)`, `", ".join(xs)`, `&& || !` for `and or not`.

## Json
`json(text)` gives a `Json`. Read it directly, no match needed: `j["key"][0]` (missing gives `Null`), `j.text()` (a string's text, `""` for null, other values as JSON), `j.int()`, `j.dec()`, `j.float()` (0 unless a JSON number, so `"7"` gives 0), `j.is_null()`, `j.has(k)`, `j.keys()`, `j.len()`, `for x in j["items"]:`, `if j["ok"]:` (null, false, 0, "" and [] are false), `j["tier"] == "gold"`, `m[j["id"]]`.
To tell kinds apart, match on `Null`, `Bool(b)`, `Num(n)` (f64), `Str(s)`, `Arr(xs)`, `Obj(m)`. `str(j)` writes compact JSON. `x.json()` turns any value into Json. A map literal whose values differ in type is a Json object with keys in written order: `str({"id": 1, "name": n})` gives `{"id":1,"name":"ann"}`.

## Regex
Python syntax: `\d \w \s . [a-z] [^x] * + ? {2,4} *? ( ) (?: ) | ^ $ \b`. Write `"\d+"` (unknown escapes keep their backslash).
`s.re_full(p)` (whole string) and `s.re_find(p)` (first match) give `[whole, group1, ...]`, or `[]` when there is no match; `s.re_all(p)` gives one such list per match; `s.re_list(p)` gives each match's text (a tokenizer: `s.re_list("\d+|[A-Z]+\d*|\S")`); `s.re_sub(p, "<b>\1</b>")` or `s.re_sub(p, g => g[1].upper())`; `s.re_split(p)`.

## Dates
`date("2024-02-29")` gives a `Date` with `days` (since 1970-01-01), `add_days(n)`, `year month day weekday` (1 = Monday); `str(d)` is `YYYY-MM-DD`. `csv(text)` gives `[[str]]`, handling quoted fields and `""`.

## Numbers
`dec` is an exact decimal (9 places) for money: `"19.99".dec()`, `0.dec()`, `+ - * /`, unary `-`, and ints mix in (`0 - p`, `p * 2`, `t += 1`, `p > 0`), `sum`, `fmt(2)` and `round(2)` (round half to even), `float int round`.
`big` is an exact integer of any size: `"123...".big()`, `n.big()`, with `+ - *` and comparisons.
`heap()` is a min-heap of any comparable values: `h.push(x)`, `h.pop()` (smallest; tuples compare in order), `first len is_empty`.

## Tools
`brevi build f.bv -o app --release` builds an optimized binary. Runtime errors print `{"e":"runtime","line":N,"msg":...}`.
`brevi emit f.bv` shows the Rust. `--explain` on any command adds the guide lines under each error. `brevi check f.bv --fast` skips rustc. `del fn name` in a patch file removes a function.
A fixed typo prints `{"line":N,"fixed":"totl -> total"}`.

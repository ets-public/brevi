# Brevi guide: write Python

Brevi compiles ordinary Python 3 to a native program. Write the program as you would in Python, in the `.bv` file. `check` builds it and runs the tests; when it reports an error, it names the line and usually the fix.

## What works as in Python
- `def`, `class` with `__init__` and methods, `global`, `nonlocal`, `lambda`, `if __name__ == "__main__":`, `try`/`except ValueError` (or `Exception`), `raise ValueError(msg)`, `assert`.
- `if`/`elif`/`else`, `for`, `while`, `break`, `continue`, `a if c else b`, tuple unpacking (`a, b = b, a`), list/dict/set comprehensions, Python truth (`if xs:`, `if not s:`).
- Built-ins: `print` (with `sep`, `end`, `*xs`), `input`, `len`, `range`, `enumerate`, `zip` (two lists), `sorted`/`min`/`max` (with `key=`, `reverse=`), `sum`, `any`, `all`, `abs`, `round`, `divmod`, `int`, `float`, `str`, `ord`, `chr`, `list`, `set`, `dict(pairs)`, `map`, `filter`, `reversed`, `next`.
- Strings: f-strings with format specs (`f"{x:.2f} {n:>5} {s:<10}"`), `%` formatting, `.format`, slicing, `split`, `strip`, `join`, `replace`, `find`, `count`, `startswith`, `upper`, `lower`, `title`, `zfill`, `center`, `isdigit`, `isalpha`, `splitlines`.
- Lists, dicts (insertion order kept), sets: `append`, `pop`, `insert`, `extend`, `sort(key=...)`, `index`, `in`, `get`, `items`, `keys`, `values`, `setdefault`, `add`, slicing, negative indexes.
- Modules: `sys` (`sys.stdin`, `sys.stdin.read()`, `sys.stdout.write`, `sys.exit`), `math`, `re`, `heapq`, `csv.reader`, `decimal.Decimal`, `datetime.date.fromisoformat`, and from `collections`: `defaultdict`, `Counter` (with `most_common`), `deque`, `OrderedDict`. Import lines are fine.

## Rules that differ from Python
- One type per variable, list and dict key/value. Use a tuple or a class for a record with mixed fields; never a list like `["a", 1]`.
- int and float do not mix silently: write `float(n)` or `int(x)` where Python would convert for you (a float literal next to a float is fine).
- An empty `{}` or `[]` whose contents cannot be worked out needs a type the first time: write `stock: {str: int} = {}` or `names: [str] = []`.
- Not supported yet: default argument values (`def f(x, k=2)`), `yield`, class inheritance, `@dataclass` and other decorators, `*args`/`**kwargs`, `eval`/`exec`, `getattr`/`setattr`, other third-party or stdlib modules.

## Extras when they help
Brevi also has short forms you may use: `"{x}"` interpolates without the `f`, `xs.map(x => x * 2)`, `xs.filter(f)`, `xs.sum()`, `s.ints()` (all integers in a string), `stdin()` (all input), `lines()` (input lines). For details on any name, call `guide` with that word.

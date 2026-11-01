# Brevi guide: write Python

Brevi compiles plain Python 3 to a fast native program. Write ordinary, idiomatic Python in `.bv` files, exactly as you would in `.py` files: `def`, classes (with inheritance and `@dataclass`), default and keyword arguments, comprehensions, generators, f-strings, `try`/`except`, and imports between your own files (`from stock import Stock` reads `stock.bv`).

The standard library parts that work: `sys`, `math`, `re`, `heapq`, `csv`, `decimal`, `datetime.date`, and `collections` (`defaultdict`, `Counter`, `deque`, `OrderedDict`). No third-party packages, `eval`, `exec` or `getattr`.

The one real difference: each variable, list and dict holds one type. Use a tuple, class or dataclass for a record with mixed fields, not a list like `["a", 1]`. Where an empty `{}` or `[]` cannot be worked out, `check` says so; then write `stock: dict[str, int] = {}` style annotations only there.

`check` builds the program and runs the tests. Its errors name the line and usually the fix. For details on a name, call `guide` with that word.

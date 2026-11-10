# re.Match: m[g], group() with __index__ and int subclasses, and str.format_map reading its mapping lazily.
import re
from collections import defaultdict

m = re.match(r"(?P<a>a)(b)?", "ac")
print(m[0], m[1], m[2], m["a"])
class Index:
    def __init__(self, v): self.v = v
    def __index__(self): return self.v
class MyInt(int): pass
print(m.group(Index(1)), m.group(MyInt(1)), m.group(True))
for bad in (3, -1, 2**70, "zz"):
    try:
        m.group(bad)
    except IndexError as e:
        print("IndexError", e)
try:
    m[0] = "x"
except TypeError as e:
    print("TypeError", e)
print("{a}-{b}".format_map(defaultdict(lambda: "?", a="A")))
try:
    "{0}".format_map({})
except ValueError as e:
    print("ValueError", e)

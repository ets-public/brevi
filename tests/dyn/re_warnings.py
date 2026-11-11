# re issues CPython's DeprecationWarning (positional count/maxsplit) and FutureWarning (possible set
# operations) through the warnings module, so filters apply: "error" makes them raise.
import re, warnings
warnings.simplefilter("error")
for f in (lambda: re.sub("a", "b", "aaa", 1), lambda: re.subn("a", "b", "aaa", 1, 0), lambda: re.split(":", "a:b", 1),
          lambda: re.compile(r"[0-9--1]"), lambda: re.compile(r"[[a]"), lambda: re.compile(r"[a&&b]"), lambda: re.compile(r"[a||b]"),
          lambda: re.compile(r"[a~~b]"), lambda: re.compile(r"[%--1]"), lambda: re.compile(rb"[a--b]"), lambda: re.sub("a", "b", "aaa", count=1)):
    try:
        print(repr(f()))
    except Warning as e:
        print(type(e).__name__, e, isinstance(e, DeprecationWarning), isinstance(e, FutureWarning))
try:
    re.sub("a", "b", "aaa", 1, count=1)
except TypeError as e:
    print("TypeError", e)
try:
    re.split("a", "b", 1, 0, 3)
except TypeError as e:
    print("TypeError", e)
warnings.simplefilter("ignore")
print(re.sub("a", "b", "aaaaa", re.I), re.split("a", "xaxax", re.I))

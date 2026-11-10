# Review l08: functools.wraps copies what CPython's update_wrapper copies, and a def's __module__ and
# __dict__ read as in CPython. (Reading __annotations__ sends a program to CPython; see dynpy/typing.rs.)
import functools


def deco(f):
    @functools.wraps(f)
    def inner(*a, **k):
        return f(*a, **k)
    return inner


@deco
def stacked(x):
    "doc here"
    return x * 2


print(stacked.__name__, stacked.__doc__, stacked(5), stacked.__wrapped__.__name__)
print(stacked.__module__, stacked.__qualname__)


def annotated(a: int, b: "str" = "x", *rest: float, key: bool = False) -> list:
    return [a, b, rest, key]


w = deco(annotated)
print(w(1, "y", 2.5, key=True), w.__name__, w.__qualname__, repr(w).split(" at ")[0])

# user attributes are copied through __dict__, and writes through __dict__ stick
annotated.tag = "t1"
w2 = deco(annotated)
print(w2.tag, sorted(w2.__dict__))
w2.__dict__["extra"] = 42
print(w2.extra)
w2.other = 7
print(w2.__dict__["other"], "other" in w2.__dict__, w2.__dict__ is w2.__dict__)
print(getattr(w2, "missing", "dflt"), getattr(w2, "extra", "dflt"), hasattr(w2, "tag"))

# update_wrapper directly, and assigned/updated limits
def target(): "target doc"
def wrapper(): pass
target.mark = 1
r = functools.update_wrapper(wrapper, target)
print(r is wrapper, wrapper.__name__, wrapper.__doc__, wrapper.mark, wrapper.__wrapped__ is target)
def wrapper2(): pass
functools.update_wrapper(wrapper2, target, assigned=("__doc__",), updated=())
print(wrapper2.__name__, wrapper2.__doc__, hasattr(wrapper2, "mark"))

# stacked wraps keep a chain of __wrapped__
@deco
@deco
def twice(x):
    "twice doc"
    return x + 1
print(twice(1), twice.__doc__, twice.__wrapped__.__wrapped__.__name__)

# lru_cache wrappers carry cache_info and the wrapped function's metadata
@functools.lru_cache(maxsize=32)
def fib(n: int) -> int:
    "fib doc"
    return n if n < 2 else fib(n - 1) + fib(n - 2)
print(fib(20), fib.cache_info(), fib.__doc__, fib.__wrapped__.__name__)
print(fib.cache_parameters())
fib.cache_clear()
print(fib.cache_info())

# least recently used results are dropped once maxsize is reached
calls = []
@functools.lru_cache(maxsize=2)
def sq(x):
    calls.append(x)
    return x * x
for x in [1, 2, 1, 3, 2, 1]:
    sq(x)
print(calls, sq.cache_info())

# keyword arguments are keyed apart from positional ones; typed keys 1 and 1.0 apart
@functools.lru_cache(maxsize=None)
def kw(*a, **k):
    return (a, sorted(k.items()))
print(kw(1, "k", 2), kw(1, k=2), kw.cache_info())
@functools.lru_cache(typed=True)
def ty(x):
    return type(x).__name__
print(ty(1), ty(1.0), ty(True), ty.cache_info())
@functools.lru_cache(maxsize=0)
def none(x):
    return x
print(none(1), none(1), none.cache_info())
@functools.cache
def c(x):
    return x
c(1); c(1)
print(c.cache_info(), c.cache_parameters())

# __name__ assignment shows in repr; non-str is CPython's TypeError
def named():
    pass
named.__name__ = "renamed"
named.__qualname__ = "Q.renamed"
print(named.__name__, repr(named).split(" at ")[0])
try:
    named.__name__ = 3
except TypeError as e:
    print("TypeError", e)

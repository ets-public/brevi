# PEP 695 type parameters in a dynamic build: bounds, constraints (any length), defaults, NoDefault,
# isinstance with TypeVar, TypeVarTuple and ParamSpec, type aliases and generic classes.
from typing import NoDefault, ParamSpec, TypeVar, TypeVarTuple


def f[T, U: int, V: (int,), W: (), *Ts, **P, X = str](a: T, b=1) -> U:
    return a


t, u, v, w, ts, p, x = f.__type_params__
print(f(5), f.__name__, f.__qualname__, [k.__name__ for k in f.__type_params__])
print(t.__bound__, u.__bound__, v.__constraints__, w.__constraints__, t.__constraints__)
print(t.__default__ is NoDefault, x.__default__, x.has_default(), t.has_default(), NoDefault)
print(isinstance(t, TypeVar), isinstance(ts, TypeVarTuple), isinstance(p, ParamSpec))
print(isinstance(t, ParamSpec), isinstance(ts, TypeVar), isinstance(p, TypeVarTuple), isinstance(3, TypeVar))
print(t | int, int | t)


def lazy[T: Missing]():
    pass


(m,) = lazy.__type_params__
for _ in range(2):
    try:
        m.__bound__
    except NameError as e:
        print("NameError", e)
Missing = float
print(m.__bound__)

type Pair[K] = tuple[K, K]
print(Pair, Pair.__name__, [k.__name__ for k in Pair.__type_params__], Pair.__value__)


class Box[T]:
    def __init__(self, item: T):
        self.item = item

    def get(self) -> T:
        return self.item


b = Box(3)
print(b.get(), Box.__name__, Box.__qualname__, Box.__parameters__, Box.__type_params__ == Box.__parameters__)
print(type(b).__name__, isinstance(b, Box))

# A stand-in for a third-party library (tests/bridge): the dynamic build cannot read this package, so CPython
# runs it inside the program, as it would run requests or pydantic.

import inspect


class LibError(Exception):
    def __init__(self, msg, code):
        super().__init__(msg)
        self.code = code

    def __str__(self):
        return f"LibError[{self.code}]: {self.args[0]}"


class NotFound(LibError, KeyError):
    pass


class Model:
    """Fields from annotations, checked on construction (like a tiny pydantic)."""

    def __init_subclass__(cls, **kw):
        super().__init_subclass__(**kw)
        cls._fields = list(inspect.get_annotations(cls))

    def __init__(self, **kw):
        for f in self._fields:
            if f not in kw and not hasattr(type(self), f):
                raise LibError(f"missing field {f}", 422)
            setattr(self, f, kw.get(f, getattr(type(self), f, None)))

    def dump(self):
        return {f: getattr(self, f) for f in self._fields}

    def __repr__(self):
        return type(self).__name__ + "(" + ", ".join(f"{f}={getattr(self, f)!r}" for f in self._fields) + ")"


class Store:
    def __init__(self):
        self.items = []
        self.meta = {"n": 0}

    def add(self, x):
        self.items.append(x)
        self.meta["n"] += 1
        return self

    def get(self, i):
        if i >= len(self.items):
            raise NotFound("no item", 404)
        return self.items[i]

    def __len__(self):
        return len(self.items)

    def __iter__(self):
        return iter(self.items)

    def __contains__(self, x):
        return x in self.items

    def __getitem__(self, i):
        return self.items[i]

    def __enter__(self):
        print("enter")
        return self

    def __exit__(self, t, v, tb):
        print("exit", t.__name__ if t else None, v)
        return t is not None and issubclass(t, ValueError)


def apply(f, xs, **kw):
    return [f(x, **kw) for x in xs]


def fill(xs, d):
    xs.append(len(xs))
    d["filled"] = True


def countdown(n):
    while n > 0:
        print("yield", n)
        yield n
        n -= 1


def total(it):
    return sum(it)


def describe(x):
    return f"{type(x).__name__}:{x!r}"


class Money:
    def __init__(self, cents):
        self.cents = cents

    def __add__(self, o):
        return Money(self.cents + (o.cents if isinstance(o, Money) else int(o * 100)))

    __radd__ = __add__

    def __lt__(self, o):
        return self.cents < o.cents

    def __eq__(self, o):
        return isinstance(o, Money) and self.cents == o.cents

    def __hash__(self):
        return hash(self.cents)

    def __format__(self, spec):
        return format(self.cents / 100, spec)

    def __repr__(self):
        return f"Money({self.cents})"

    def __bool__(self):
        return self.cents != 0


# What a library reads off a function the program hands it (tests/bridge/func_introspect.py).
def about(f):
    return (f.__name__, f.__qualname__, f.__doc__, f.__module__, inspect.isfunction(f), inspect.ismethod(f),
            inspect.isroutine(f), callable(f), type(f).__name__, inspect.iscoroutinefunction(f),
            inspect.isgeneratorfunction(f), f.__defaults__, getattr(f, "tag", None))


# A function put in a class binds as a method, as a def does.
def as_method(f):
    class Holder:
        m = f
    return Holder().m()


def registry(*fs):
    seen = {}
    for f in fs:
        seen[f] = seen.get(f, 0) + 1
    return sorted(seen.values()), fs[0] in seen


def wrap(f):
    import functools

    @functools.wraps(f)
    def w(*a, **k):
        return f(*a, **k)
    return w

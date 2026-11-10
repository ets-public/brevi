def show(**kw):
    return sorted(kw.items())


def g(a, b=5, *args, c=7, **kw):
    return (a, b, args, c, kw)


def h(a):
    return a


def tryit(f):
    try:
        print(f())
    except TypeError as e:
        print("TypeError", e)


tryit(lambda: show(**{"x": 1}, x=9))
tryit(lambda: show(x=9, **{"x": 1}))
tryit(lambda: show(**{"x": 1, "y": 2}, y=9))
tryit(lambda: show(**{"a": 1, "b": 2}, b=3, a=4))
tryit(lambda: show(**{"x": 1}, y=2))
tryit(lambda: g(1, **{"a": 2}))
tryit(lambda: g(1, b=2, **{"b": 3}))
tryit(lambda: g(1, **{"a": 2, "b": 3}, b=4))
tryit(lambda: g(1, 2, 3, **{"x": 1}, x=2))
tryit(lambda: h(**{"a": 1}, a=2))
tryit(lambda: sorted(**{"x": 1}, x=2))
tryit(lambda: dict(**{"x": 1}, x=2))


class C:
    def m(self, **kw):
        return sorted(kw.items())

    @staticmethod
    def s(**kw):
        return sorted(kw.items())

    @classmethod
    def cm(cls, **kw):
        return sorted(kw.items())


tryit(lambda: C().m(**{"x": 1}, x=9))
tryit(lambda: C.s(**{"x": 1}, x=9))
tryit(lambda: C.cm(**{"x": 1}, x=9))
tryit(lambda: C.m(**{"x": 1}, x=9))


def outer():
    def inner(**kw):
        return sorted(kw.items())

    tryit(lambda: inner(**{"x": 1}, x=9))


outer()

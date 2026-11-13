# PEP 479: StopIteration escaping a generator body becomes RuntimeError, chained from it.
import asyncio


def show(label, f):
    try:
        r = f()
        print(label, "ok", r)
    except BaseException as e:
        c = e.__cause__
        print(label, type(e).__name__, e.args, type(c).__name__, c.args if c is not None else None,
              e.__context__ is c, e.__suppress_context__)


def g1():
    yield 1
    raise StopIteration("boom")


def g2():
    yield 1
    next(iter([]))


def g3():
    yield from g1()


def g4():
    yield 1
    return 7


def g5():
    v = yield from g4()
    yield v


class MyStop(StopIteration):
    pass


def g6():
    yield 1
    raise MyStop(3)


def g7():
    try:
        yield 1
    except ValueError:
        pass
    raise StopIteration


def g8():
    yield 1
    raise StopAsyncIteration


show("raise", lambda: list(g1()))
show("next", lambda: list(g2()))
show("yield from", lambda: list(g3()))
show("return", lambda: list(g4()))
show("return yf", lambda: list(g5()))
show("subclass", lambda: list(g6()))
show("for loop", lambda: [x for x in g1()])
show("genexp", lambda: list(next(it) for it in [iter([1]), iter([])]))
show("sum genexp", lambda: sum(next(iter([])) for _ in range(2)))
show("sync StopAsync", lambda: list(g8()))


def thrown():
    g = g7()
    next(g)
    return g.throw(StopIteration("t"))


show("throw", thrown)


def thrown_in():
    g = g4()
    next(g)
    return g.throw(StopIteration("in"))


show("throw at yield", thrown_in)


def caught():
    out = []
    try:
        for x in g1():
            out.append(x)
    except RuntimeError as e:
        out.append(str(e))
    return out


show("caught", caught)


def same_cause():
    s = StopIteration("same")

    def g():
        yield 1
        raise s

    try:
        list(g())
    except RuntimeError as e:
        return e.__cause__ is s


show("same cause", same_cause)


def gen_close():
    def g():
        try:
            yield 1
        finally:
            raise StopIteration("fin")

    it = g()
    next(it)
    return it.close()


show("close", gen_close)


async def ag1():
    yield 1
    raise StopIteration("a")


async def ag2():
    yield 1
    raise StopAsyncIteration("b")


async def collect(ag):
    return [x async for x in ag]


show("agen StopIteration", lambda: asyncio.run(collect(ag1())))
show("agen StopAsyncIteration", lambda: asyncio.run(collect(ag2())))


def cm_stop():
    import contextlib

    @contextlib.contextmanager
    def cm():
        yield

    try:
        with cm():
            raise StopIteration("cm")
    except StopIteration as e:
        return "StopIteration " + str(e)


show("contextmanager", cm_stop)

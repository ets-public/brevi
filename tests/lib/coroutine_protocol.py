# The coroutine object protocol without an event loop, as CPython's test_coroutines drives it: send/throw/close,
# __await__, reuse and re-entry errors, PEP 479, `async with`/`async for` errors, and the types module's classes.
import copy
import types
import warnings


def run(coro):
    out = []
    while True:
        try:
            out.append(coro.send(None))
        except StopIteration as e:
            return out, (e.args[0] if e.args else None)


def outer():
    async def inner():
        return 7
    return inner


c = outer()()
print(type(c) is types.CoroutineType, c.__class__ is types.CoroutineType, isinstance(c, types.CoroutineType))
print(c.__qualname__, c.__name__, repr(c).startswith("<coroutine object outer.<locals>.inner at 0x"))
print(type(lambda: 0) is types.FunctionType)


def gen():
    yield 1


async def agen():
    yield 1


print(type(gen()) is types.GeneratorType, type(agen()) is types.AsyncGeneratorType)
print(run(c))
for op in ("send", "throw"):
    try:
        c.send(None) if op == "send" else c.throw(KeyError("k"))
    except RuntimeError as e:
        print(op, e)
c.close()


async def reenter():
    me.send(None)

me = reenter()
try:
    me.send(None)
except ValueError as e:
    print("ValueError", e)


async def stops():
    raise StopIteration

try:
    run(stops())
except RuntimeError as e:
    print(e, type(e.__cause__).__name__)


async def spam():
    return "spam"

co = spam()
w = co.__await__()
print(iter(w) is w, "coroutine_wrapper" in repr(w), type(w).__name__)
try:
    w.send(None)
except StopIteration as e:
    print("stop", e.args)
for f in (lambda: w.send(None), lambda: next(w), lambda: w.throw(Exception("x"))):
    try:
        f()
    except RuntimeError as e:
        print(e)
w.close()

co = spam()
def yf():
    yield from co
try:
    list(yf())
except TypeError as e:
    print(e)
co.close()

co = spam()
for f in (copy.copy, copy.deepcopy):
    try:
        f(co)
    except TypeError as e:
        print(e)
print("__await__" in dir(co), "__iter__" in dir(co.__await__()))
co.close()


class NoExit:
    async def __aenter__(self):
        return self


class BadExit:
    async def __aenter__(self):
        return self

    def __aexit__(self, *e):
        return 444


class RaisingExit:
    async def __aenter__(self):
        return self

    async def __aexit__(self, *e):
        1 / 0


for cm in (NoExit, BadExit, RaisingExit):
    async def body():
        async with cm():
            raise RuntimeError("inside")
    try:
        run(body())
    except (TypeError, ZeroDivisionError) as e:
        print(type(e).__name__, e, "|", type(e.__context__).__name__)


class BadAwait:
    def __aiter__(self):
        return self

    def __anext__(self):
        return self

    def __await__(self):
        1 / 0


async def loop():
    async for _ in BadAwait():
        pass

try:
    run(loop())
except TypeError as e:
    print(e, type(e.__cause__).__name__)


async def bad_await():
    await 1

try:
    run(bad_await())
except TypeError as e:
    print(e)

try:
    try:
        raise KeyError("first")
    except KeyError:
        [][1]
except IndexError as e:
    print(type(e.__context__).__name__)

with warnings.catch_warnings(record=True) as ws:
    warnings.simplefilter("always")
    spam()
print([(w.category.__name__, str(w.message)) for w in ws])

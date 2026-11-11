# A warning names the program's line, as CPython's does: warnings.warn in a function, after a call returns,
# in a generator across yields, in a lambda, in a function a library calls back, a library's warning whose
# stacklevel points at the program, a loop's line again, and the default filters' once-per-line.
import ast
import os
import timeit
import warnings


def show(message, category, filename, lineno, file=None, line=None):
    print(lineno, os.path.basename(filename), category.__name__, str(message)[:40])


warnings.showwarning = show


def inner():
    warnings.warn("in inner")


def outer():
    x = 1
    inner()
    warnings.warn("in outer after inner")


def gen():
    warnings.warn("gen start")
    yield 1
    warnings.warn("gen after yield")
    yield 2


lam = lambda: warnings.warn("lambda")


def cb():
    warnings.warn("callback")


class K:
    warnings.warn("class body")


with warnings.catch_warnings():
    warnings.simplefilter("always")
    warnings.warn("top")
    outer()
    g = gen()
    next(g)
    warnings.warn("between")
    next(g)
    lam()
    timeit.timeit(cb, number=1)
    ast.Num
    k = 0
    while k < 2:
        k += 1
    for i in range(2):
        warnings.warn("loop")
    warnings.warn(DeprecationWarning("as instance"))
    warnings.warn(
        "over two lines, the call on the first")
    try:
        warnings.warn("x", 5)
    except TypeError as e:
        print("TypeError", e)
with warnings.catch_warnings():
    warnings.simplefilter("default")
    for i in range(3):
        warnings.warn("once here")
    warnings.warn("once here")
with warnings.catch_warnings():
    for i in range(3):
        warnings.warn("DeprecationWarning shown for __main__", DeprecationWarning)
with warnings.catch_warnings():
    warnings.simplefilter("error")
    try:
        inner()
    except UserWarning as e:
        print("raised", e)
print(warnings.warn.__name__)

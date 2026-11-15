# A library CPython runs sees the program's functions as CPython's: names, docs, defaults, attributes, kind
# (inspect.isfunction, ismethod, isgeneratorfunction), binding in a class, equality and hashing, functools.wraps.
import inspect
from brlib import about, as_method, kinds, registry, wrap


def area(w, h=2):
    "Width times height."
    return w * h


def gen(n):
    yield n


area.tag = "shape"


class Box:
    def size(self):
        "The box's size."
        return 3

    def nested(self):
        def inner():
            return 1
        return inner


b = Box()
print(about(area))
print(about(gen))
print(about(lambda x: x))
print(about(b.size))
print(about(b.nested()))
print(as_method(lambda self: type(self).__name__))
print(registry(area, area, gen))
w = wrap(area)
print(w.__name__, w.__doc__, w(3), w.__wrapped__ is area, w.tag)
print(inspect.getdoc(area), inspect.getdoc(b.size))
print(inspect.isfunction(area), inspect.ismethod(b.size), inspect.getmodule(area).__name__)
print(kinds(area, b.size))

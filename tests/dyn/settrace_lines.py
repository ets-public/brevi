# sys.settrace in a dynamic build: call, line, return and exception events, as CPython 3.13 sends them
import sys


def show(frame, event, arg):
    if event == "exception":
        print(event, frame.f_code.co_name, frame.f_lineno - frame.f_code.co_firstlineno, arg[0].__name__)
    else:
        print(event, frame.f_code.co_name, frame.f_lineno - frame.f_code.co_firstlineno)
    return show


def deco(h):
    return h


@deco
def add(a, b):
    c = a + b
    if c > 3:
        c -= 1
    elif c == 0:
        pass
    else:
        c = 0
    return c


def pick(x):
    match x:
        case 1:
            return "one"
        case 2 | 3:
            return "few"
        case _:
            return "many"


def guarded(x):
    try:
        assert x > 0
        return x
    finally:
        x = 0


class Box:
    size = 2

    def __init__(self, v):
        self.v = v

    @staticmethod
    def make(v):
        return Box(v)

    def get(self):
        return self.v * self.size


def outer(n):
    def inner(k):
        return k + 1
    return inner(n) + Box.make(n).get()


sys.settrace(show)
print(add(1, 2), add(3, 4), add(0, 0))
print(pick(1), pick(3), pick(9))
print(guarded(5))
try:
    guarded(0)
except AssertionError:
    print("caught")
print(outer(2))
sys.settrace(None)
print(sys.gettrace())

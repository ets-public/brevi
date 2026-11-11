# pickle saves the program's functions and classes by reference (module and qualified name), checks the name
# finds the same object, and loads the very object again; total_ordering's helpers are functools' shared ones.
import functools
import pickle


def f(x):
    return x + 1


class C:
    def m(self):
        return 1

    class D:
        pass


for proto in range(pickle.HIGHEST_PROTOCOL + 1):
    print(proto, [pickle.loads(pickle.dumps(o, proto)) is o for o in (f, C, C.m, C.D, int)])
print(pickle.loads(pickle.dumps(f))(2))
print([type(x).__name__ for x in pickle.loads(pickle.dumps([f, C, list, 3]))])


def g():
    def h():
        pass
    return h


try:
    pickle.dumps(g())
except AttributeError as e:
    print("AttributeError", e)

k = f


def f(x):
    return x


try:
    pickle.dumps(k)
except pickle.PicklingError as e:
    print("PicklingError", str(e).split(": ", 1)[1])


@functools.total_ordering
class O:
    def __init__(self, v):
        self.v = v

    def __lt__(self, o):
        return self.v < o.v

    def __eq__(self, o):
        return self.v == o.v


@functools.total_ordering
class P:
    def __init__(self, v):
        self.v = v

    def __le__(self, o):
        return self.v <= o.v


class Sub(O):
    def __lt__(self, o):
        return True


print(O.__gt__ is P.__lt__, O.__ge__ is functools.total_ordering(type("Q", (O,), {})).__ge__)
for m in (O.__gt__, O.__le__, P.__lt__, P.__ge__):
    print(m.__module__, m.__qualname__, m.__name__)
print(Sub(1) > Sub(2), Sub(1) <= Sub(2), O(1) > O(2), O(2) >= O(2), O(1) <= O(2))
for n in ("__gt__", "__le__", "__ge__"):
    print(n, pickle.loads(pickle.dumps(getattr(O, n), 0)) is getattr(O, n))

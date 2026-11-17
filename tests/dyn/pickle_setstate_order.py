# pickle.loads of the program's objects whose class gives itself its state (__setstate__, __getattr__,
# __setattr__): the program's code runs as CPython runs it, in the order CPython's Unpickler reads the BUILDs.
import copy
import pickle
from dataclasses import dataclass, field

log = []


class Node:
    def __init__(self, name, kids=()):
        self.name = name
        self.kids = list(kids)

    def __setstate__(self, state):
        # the kids were given their state already, as CPython gives it before this one's
        log.append((state["name"], [k.name for k in state["kids"]]))
        self.__dict__.update(state)


tree = Node("root", [Node("a", [Node("a1"), Node("a2")]), Node("b")])
for proto in range(pickle.HIGHEST_PROTOCOL + 1):
    log.clear()
    t = pickle.loads(pickle.dumps(tree, proto))
    print(proto, log, t.kids[0].kids[1].name)

# a cycle: the parent's state holds the child, whose state holds the parent before it has its state
class Cyc:
    def __setstate__(self, state):
        log.append((state["n"], hasattr(state["other"], "n")))
        self.__dict__.update(state)


p, c = Cyc(), Cyc()
p.n, c.n, p.other, c.other = "p", "c", c, p
for proto in (0, 2, 4):
    log.clear()
    q = pickle.loads(pickle.dumps([p, c, p], proto))
    print(proto, log, q[0] is q[2], q[0].other is q[1], q[1].other is q[0])


# a state that is not a dict, and a __setstate__ that raises: the error comes out of pickle.loads
class Pair:
    def __init__(self, a, b):
        self.a, self.b = a, b

    def __getstate__(self):
        return (self.a, self.b)

    def __setstate__(self, st):
        if st[0] < 0:
            raise ValueError("negative")
        self.a, self.b = st


print(vars(pickle.loads(pickle.dumps(Pair(1, [2, 3])))))
try:
    pickle.loads(pickle.dumps([Pair(1, 2), Pair(-1, 2)]))
except ValueError as e:
    print("ValueError", e)


# __getattr__: pickle looks __setstate__ up once, copy twice (hasattr, then the call)
class Lazy:
    def __init__(self):
        self.x = 1

    def __getattr__(self, name):
        log.append(name)
        raise AttributeError(name)


log.clear()
z = pickle.loads(pickle.dumps(Lazy()))
print(z.x, log)


class Proxy:
    def __init__(self):
        self.x = 1

    def __getattr__(self, name):
        log.append(name)
        if name == "__setstate__":
            return lambda st: log.append(("set", sorted(st)))
        raise AttributeError(name)


log.clear()
z = pickle.loads(pickle.dumps(Proxy()))
print(log, "x" in vars(z))


# slots set through the class's own __setattr__
class Slotted:
    __slots__ = ("a", "b")

    def __init__(self):
        object.__setattr__(self, "a", 1)
        object.__setattr__(self, "b", 2)

    def __setattr__(self, name, value):
        log.append((name, value))
        object.__setattr__(self, name, value)


log.clear()
s = pickle.loads(pickle.dumps(Slotted(), 2))
print(log, s.a, s.b)


# a frozen dataclass with slots and its own __setstate__ (CPython's test_frozen_slots_pickle_custom_state)
@dataclass(frozen=True, slots=True)
class Frozen:
    foo: str
    bar: int
    called: bool = field(default=False, compare=False)

    def __setstate__(self, state):
        object.__setattr__(self, "called", True)
        object.__setattr__(self, "foo", state[0])
        object.__setattr__(self, "bar", state[1])


for proto in range(pickle.HIGHEST_PROTOCOL + 1):
    f = Frozen("a", 1)
    g = pickle.loads(pickle.dumps(f, proto))
    print(proto, g.called, f == g, copy.deepcopy(f).called)

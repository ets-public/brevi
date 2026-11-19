# Pickling classes of the program by qualified name: a class nested in a class (below protocol 4 pickle
# names it as getattr(Outer, 'Inner')) and a class made in a function under `global` (qualname 'P').
import pickle
from dataclasses import dataclass


class T:
    @dataclass(frozen=True, slots=True)
    class C:
        foo: str
        bar: int

    class D:
        class E:
            def __init__(self):
                self.x = 5


def make():
    global P

    @dataclass
    class P:
        a: int

    return P


Q = make()
print(Q.__qualname__, make.__qualname__)
for proto in range(pickle.HIGHEST_PROTOCOL + 1):
    c = T.C("a", 1)
    r = pickle.loads(pickle.dumps(c, protocol=proto))
    print(proto, r, r == c, r is c)
    e = pickle.loads(pickle.dumps(T.D.E(), protocol=proto))
    print(proto, type(e).__qualname__, e.x, pickle.loads(pickle.dumps(T.D.E, protocol=proto)) is T.D.E)
    p = pickle.loads(pickle.dumps(P(3), protocol=proto))
    print(proto, p, type(p) is P)

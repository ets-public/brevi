# __slots__ in the dynamic build: which attributes an instance may store, through plain assignment,
# setattr() and object.__setattr__; slots are inherited; a subclass without __slots__ has a __dict__
# again; a class listing "__dict__" in its slots keeps one; a string is one slot name.
class S:
    __slots__ = ("a",)

class T(S):
    __slots__ = ("b",)

class U(S):
    pass

class E(S):
    __slots__ = ()

class D(S):
    __slots__ = ("__dict__",)

class W:
    __slots__ = "only"

class L:
    __slots__ = ["x", "y"]

class K:
    __slots__ = {"p": "p's doc"}

class CA(S):
    x = 99

class Hook(S):
    def __setattr__(self, n, v):
        object.__setattr__(self, n, v * 2)

class Sup(S):
    def __setattr__(self, n, v):
        super().__setattr__(n, v)

def show(label, fn):
    try:
        print(label, "ok", fn())
    except Exception as e:
        print(label, type(e).__name__, e)

def settable(label, obj, n, v):
    show(label, lambda: setattr(obj, n, v))

# plain assignment, setattr() and object.__setattr__ all refuse a name that is no slot when there is no __dict__
try:
    S().b = 1
except Exception as e:
    print("assign", e)
settable("setattr", S(), "b", 1)
try:
    object.__setattr__(S(), "b", 1)
except Exception as e:
    print("object setattr", e)
try:
    object.__setattr__(S(), "__dict__", {})
except Exception as e:
    print("object dict", e)

s = S()
s.a = 5
print("slot read", s.a)
show("unset S.a", lambda: S().a)
show("unset W.only", lambda: W().only)

t = T()
t.a = 1
t.b = 2
print("T a b", t.a, t.b)
settable("T.c", t, "c", 3)

u = U()
u.anything = 4
print("U anything", u.anything)

e = E()
e.a = 5
print("E a", e.a)
settable("E.other", e, "other", 6)

d = D()
d.anything = 7
d.a = 8
print("D", d.anything, d.a)

w = W()
w.only = 9
print("W only", w.only)
settable("W other", w, "other", 10)

l = L()
l.x = 1
l.y = 2
print("L", l.x, l.y)
settable("L z", l, "z", 3)

k = K()
k.p = 4
print("K", k.p)
settable("K q", k, "q", 5)

ca = CA()
print("CA class attr", ca.x)
settable("CA shadow", ca, "x", 2)

h = Hook()
h.a = 10
print("Hook a", h.a)
settable("Hook z", h, "z", 1)

su = Sup()
su.a = 11
print("Sup a", su.a)
settable("Sup z", su, "z", 1)

print("has dict", hasattr(S(), "__dict__"), hasattr(T(), "__dict__"), hasattr(U(), "__dict__"), hasattr(E(), "__dict__"), hasattr(D(), "__dict__"), hasattr(W(), "__dict__"))
show("S.__dict__", lambda: S().__dict__)
print("U.__dict__", U().__dict__)
print("slots", S.__slots__, T.__slots__)

# a base without __slots__ leaves a __dict__ that a subclass with __slots__ keeps
class Plain:
    pass

class Child(Plain):
    __slots__ = ("a",)

c = Child()
c.z = 2
c.a = 1
print("child", hasattr(c, "__dict__"), c.z, sorted(c.__dict__))

# a builtin base whose instances have a __dict__ (exceptions) keeps it
class Ex(Exception):
    __slots__ = ("code",)

e = Ex()
e.other = 1
e.code = 2
print("exc", hasattr(e, "__dict__"), e.other, sorted(e.__dict__))

# @dataclass(slots=True) is enforced like a class with __slots__
from dataclasses import dataclass

@dataclass(slots=True)
class Point:
    x: int
    y: int = 0

p = Point(1)
print("Point slots", Point.__slots__)
show("Point dict", lambda: hasattr(p, "__dict__"))
p.x = 9
print("Point x", p.x)
settable("Point z", p, "z", 3)
show("Point missing", lambda: p.missing)

@dataclass(slots=True)
class WithMethod:
    def m(self):
        return 1

wm = WithMethod()
settable("WithMethod m", wm, "m", 5)
print("WithMethod call", wm.m())

# messages: a class attribute that is no slot is read-only; a slot that is also a class variable is
# refused at class creation; vars() needs a __dict__
class RO:
    __slots__ = ()
    k = 3

show("RO read", lambda: RO().k)
settable("RO k", RO(), "k", 4)
show("RO k after", lambda: RO().k)
try:
    class Conflict:
        __slots__ = ("a",)
        a = 1
except Exception as ex:
    print("conflict", type(ex).__name__, ex)
show("vars slotted", lambda: vars(S()))

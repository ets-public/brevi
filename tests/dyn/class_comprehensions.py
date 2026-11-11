# Comprehensions in a class body (#442): their variables are plain locals, and past the first iterable they
# see enclosing functions, globals and builtins, never the class's own names.
import enum

a = "g"


class D:
    a = 1
    q = [i for i in range(2)]
    r = [a for i in range(2)]
    s = {k: a + k for k in "xy" if a}
    t = [x for x in [a, a]]
    u = {j: [k for k in range(j)] for j in range(3)}
    v = {a for _ in range(3)}
    w = sum(i for i in range(4))


print(D.q, D.r, D.s, D.t, D.u, D.v, D.w, D.a)


def outer():
    z = 5

    class C:
        z = 99
        w = [z + i for i in range(2)]
        n = len([i for i in range(z)])

    return C.w, C.n


print(outer())


class Color(enum.Enum):
    RED = 1
    GREEN = 2
    names = [c for c in "ab"]


print(list(Color))


class L:
    a = 3
    x = [a * j for j in range(2)]
    y = locals()["x"]


print(L.x, L.y)


class Missing:
    try:
        q = [nope for _ in range(1)]
    except NameError as e:
        q = str(e)


print(Missing.q)

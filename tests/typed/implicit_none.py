# A def with no return gives None, whatever its last line is. Only `return` gives a value.
def plain(x):
    x + 1


def length(xs):
    len(xs)


def branches(x):
    if x > 0:
        x * 2
    else:
        x * 3


def upper(t):
    t.upper()


def name_only(x):
    x


def text_only(x):
    "not a value"


def list_value(x):
    [x]


def tried(x):
    try:
        x + 1
    except ValueError:
        x + 2


def add(xs):
    xs.append(1)


print(plain(1), length([1]), branches(1), branches(-1), upper("a"))
print(name_only(3), text_only(3), list_value(3), tried(3))
ys = []
print(add(ys), ys)


class C:
    def __init__(self):
        self.n = 1

    def bump(self):
        self.n + 1

    def real(self):
        return self.n + 1


print(C().bump(), C().real())


# A value on some paths only: the others give None.
def some(x):
    if x > 0:
        return x
    x + 5


def fails(x):
    if x > 0:
        return x
    raise ValueError("no")


def every(x):
    if x > 0:
        return 1
    else:
        return 2


print(some(1), some(-1), fails(1), every(0))
print((lambda x: x + 1)(1), sorted([2, 1], key=lambda v: -v))
print(list(map(lambda v: v * 2, [1, 2])))

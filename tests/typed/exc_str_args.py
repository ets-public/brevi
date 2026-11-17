# str() of an error object of the program is made of its args, as in CPython: what super().__init__ got, or
# with no such call the positional args of the call; set once, so a field changed later does not change it.
class Pair(Exception):
    def __init__(self, a, b):
        self.s = a + b


class Sum(Exception):
    def __init__(self, a, b):
        super().__init__(a + b)
        self.a = a


class Coded(Exception):
    def __init__(self, code):
        super().__init__(f"code {code}")
        self.code = code


class Named(Exception):
    def __init__(self, m):
        super().__init__(m)
        self.m = m

    def __repr__(self):
        return "Named!"


class Two(Exception):
    def __init__(self, a, b):
        Exception.__init__(self, a, b)
        self.a = a


class Empty(Exception):
    def __init__(self, v):
        super().__init__()
        self.v = v


class Late(Exception):
    def __init__(self, item, qty):
        self.item = item
        self.qty = qty


def take(n):
    if n > 2:
        raise Late("bolt", n)
    return n


try:
    raise Pair(1, 2)
except Pair as e:
    print(str(e), e.s)
try:
    raise Sum(1, 2)
except Sum as e:
    print(str(e), e.a)
try:
    raise Coded(7)
except Coded as e:
    print(e, e.code)
    print(f"got {e}")
try:
    raise Named("orig")
except Named as e:
    e.m = "changed"
    print(str(e), repr(e), e.m)
try:
    raise Two("x", 2.5)
except Two as e:
    print(str(e), e.a)
try:
    raise Empty(True)
except Empty as e:
    print(repr(str(e)), e.v)
for i in range(5):
    try:
        print(take(i))
    except Late as e:
        print("late", e, e.item, e.qty)
try:
    raise Two("y", 3.5)
except Two as e:
    print(repr(e), [e])
try:
    raise Pair(1, 5)
except Pair as e:
    print(repr(e))
try:
    raise Empty(False)
except Empty as e:
    print(repr(e))
try:
    raise Named("n")
except Named as e:
    print(repr(e))

# Regressions from the Counter stdlib work: a Counter subclass that defines __repr__ but not __str__ strs
# through its __repr__; @staticmethod/@classmethod and functools.wraps keep the dotted __qualname__; argument
# errors and generator/genexpr objects use the dotted __qualname__.
import functools
from collections import Counter


def test_counter_subclass_str():
    class Cnt2(Counter):
        __repr__ = lambda s: "X"

    print(Cnt2("a"))


def test_sm_qualnames():
    class T:
        @staticmethod
        def s():
            pass

        @classmethod
        def c(cls):
            pass

    print(T.s.__qualname__, T.s.__name__)
    print(T.c.__qualname__, T.c.__name__)
    print(repr(T.c))


def deco(f):
    @functools.wraps(f)
    def w(*a, **k):
        return f(*a, **k)

    return w


@deco
def top():
    pass


print(top.__qualname__, top.__name__, top.__wrapped__.__name__)


def test_arg_errors():
    class K:
        def m(self, a):
            pass

    try:
        K().m()
    except TypeError as e:
        print(e)

    def outer():
        def need(a):
            pass

        try:
            need()
        except TypeError as e:
            print(e)

    outer()


def test_gen_qualnames():
    def gen():
        yield 1

    g = gen()
    print(g.__qualname__, g.__name__)
    print(repr(g).split(" at 0x")[0])
    e = (x for x in [1])
    print(e.__qualname__, e.__name__)
    print(repr(e).split(" at 0x")[0])


test_counter_subclass_str()
test_sm_qualnames()
test_arg_errors()
test_gen_qualnames()

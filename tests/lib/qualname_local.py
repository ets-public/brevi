def outer():
    def inner():
        pass

    class Local:
        pass

    lam = lambda: 0
    print(inner.__qualname__)
    print(inner.__name__)
    print(Local.__qualname__)
    print(Local.__name__)
    print(lam.__qualname__)
    print(repr(Local))


class Outer:
    class Inner:
        pass

    def method(self):
        def h():
            pass

        class D:
            pass

        return h, D


h, D = Outer().method()
print(Outer.Inner.__qualname__)
print(Outer.Inner.__name__)
print(repr(Outer.Inner))
print(h.__qualname__)
print(h.__name__)
print(D.__qualname__)
print(D.__name__)
print(repr(D))

lam = lambda: 0
print(lam.__qualname__)


def top():
    pass


print(top.__qualname__, top.__name__)

outer()

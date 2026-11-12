class B:
    def __init_subclass__(cls, flag=False):
        pass
class A(B, flag=True):
    pass
def f():
    print({3, -1, 5})

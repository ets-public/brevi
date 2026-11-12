# The guard cannot read a class keyword argument, which the typed parser takes: that must not count as nothing
# to refuse.
class B:
    def __init_subclass__(cls, flag=False):
        pass
class A(B, flag=True):
    pass
print("made")
print({3, -1, 5})

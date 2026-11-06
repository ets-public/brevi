# The guard cannot read a metaclass, which the typed parser takes: that must not count as nothing to refuse.
class A(object, metaclass=type):
    pass
print("made")
print({3, -1, 5})

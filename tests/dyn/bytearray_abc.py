from collections.abc import Buffer, Sequence, MutableSequence, Hashable, Iterable, Sized, Container, Reversible, Collection
for v in (b'', bytearray(), memoryview(b'')):
    print(type(v).__name__, [c.__name__ for c in (Buffer, Sequence, MutableSequence, Hashable, Iterable, Sized, Container, Reversible, Collection) if isinstance(v, c)])
print(issubclass(bytearray, Buffer), issubclass(memoryview, Sequence), issubclass(bytearray, MutableSequence))

# collections.abc.ByteString warns as CPython 3.13's _DeprecateByteStringMeta does: on isinstance (not issubclass, and
# not for an instance of the exact class) and when a subclass is made, at the program's line.
import warnings
from collections.abc import ByteString, Sequence

warnings.simplefilter("ignore", DeprecationWarning)
print(isinstance(b"", ByteString), issubclass(bytes, ByteString), isinstance([], (int, ByteString)))
with warnings.catch_warnings(record=True) as w:
    warnings.simplefilter("always")
    isinstance(b"", ByteString)
    issubclass(bytearray, ByteString)
    class X(ByteString):
        def __getitem__(self, i): return 0
        def __len__(self): return 0
        a = 1

        def f(self): pass
    class Y(X): pass
    isinstance(Y(), Y)
    isinstance(Y(), X)
    isinstance(3, Sequence)
for m in w:
    print(m.category.__name__, m.lineno, m.message)

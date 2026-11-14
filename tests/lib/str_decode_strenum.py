# str(bytes, encoding[, errors]), sys.getdefaultencoding, and StrEnum / (str, Enum) members made from several
# arguments, with CPython's TypeErrors (dynamic builds)
import sys
from enum import StrEnum, Enum
print(sys.getdefaultencoding(), repr(sys.getdefaultencoding))
try:
    class A(StrEnum):
        one = '1'
        two = b'2', sys.getdefaultencoding
except TypeError as e:
    print("TE", e)
class CustomStrEnum(str, Enum):
    pass
try:
    class B(CustomStrEnum):
        one = '1'
        two = b'2', sys.getdefaultencoding
except TypeError as e:
    print("TE", e)
try:
    class C(CustomStrEnum):
        one = '1'
        two = b'2', 'ascii', 9
except TypeError as e:
    print("TE", e)
class D(CustomStrEnum):
    two = b'2', 'ascii'
print(repr(D.two))
try:
    sys.getdefaultencoding(1)
except TypeError as e:
    print("TE", e)
for args in [(b'a\xff', 'ascii'), (b'a\xff', 'ascii', 'replace'), (bytearray(b'hi'), 'utf-8'), ('x', 'utf-8'), (3, 'utf-8'), (b'x', None), (b'x', 'utf-8', None), (b'', 'nope')]:
    try:
        print(repr(str(*args)))
    except (TypeError, UnicodeDecodeError, LookupError) as e:
        print(type(e).__name__, e)
from enum import IntEnum
try:
    class I(int, Enum):
        a = 'x'
except ValueError as e:
    print("VE", e)
# a __new__ of the class's own that leaves _value_ unset: enum.py wraps the data type's error
try:
    class N(str, Enum):
        def __new__(cls, value, seq):
            m = str.__new__(cls, value)
            m.seq = seq
            return m
        a = 'x', 0
except TypeError as e:
    print("TE", e, "|", type(e.__cause__).__name__, e.__cause__)

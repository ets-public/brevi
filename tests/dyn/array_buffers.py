# array.array is a buffer: bytes, bytearray and memoryview read its bytes, and it is a MutableSequence.
import array
from collections.abc import MutableSequence, Sequence
a = array.array('h', [1, 2, 258])
print(bytes(a), bytearray(a), len(bytearray(a)))
ba = bytearray(b'xy')
ba.extend(array.array('b', b'zw'))
ba += array.array('B', [65])
print(ba)
print(bytearray(b'ab') + array.array('b', b'cd'), b'ab' + bytes(array.array('b', b'cd')))
print(bytearray(b'abc') == array.array('b', b'abc'), bytearray(b'abcd').find(array.array('b', b'cd')))
b2 = array.array('b')
b2.frombytes(b'hi')
print(b2, b2.tobytes(), bytearray(b2.tobytes()))
print(issubclass(array.array, MutableSequence), isinstance(a, MutableSequence), isinstance(a, Sequence))
print(issubclass(bytearray, MutableSequence), issubclass(bytes, MutableSequence))
x = array.array('b', b'abc')
print(b'abc' == x, x == b'abc', bytearray(b'abc') != x, array.array('b', b'bc') in b'abc', b''.join([x, b'-']))
print(bytes(array.array('h', [1, 2])), bytearray(array.array('B', [1, 2])))
ba2 = bytearray(b'0123')
ba2[1:3] = array.array('b', b'xy')
print(ba2, b'abc'.startswith(array.array('b', b'ab')), bytes(x).decode())
try:
    print(bytearray(b"a") < x, bytearray(b"b") > x)
except TypeError as e:
    print("TypeError", e)

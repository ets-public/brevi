# array.array next to bytearray and bytes: comparisons, conversions and collections.abc, as CPython answers
import array
from collections.abc import MutableSequence, Sequence, Iterable, Hashable
a = array.array('B', b'42')
tests = [
 lambda: b'42' == a, lambda: a == b'42', lambda: bytearray(b'42') == a, lambda: a == bytearray(b'42'), lambda: bytearray(b'41') < a,
 lambda: b'1' < a, lambda: bytes(array.array('h', [1, 2])), lambda: bytearray(array.array('h', [1, 2])),
 lambda: (lambda ba: (ba.extend(array.array('B', [65, 66])), ba)[1])(bytearray(b'x')),
 lambda: b'-'.join([array.array('B', b'ab'), b'c']), lambda: b'x' + a, lambda: bytearray(b'x') + a,
 lambda: b'abc'.find(array.array('B', b'b')), lambda: array.array('B', b'b') in b'abc', lambda: b'abc'.startswith(array.array('B', b'a')),
 lambda: isinstance(a, MutableSequence), lambda: isinstance(a, Sequence), lambda: isinstance(a, Hashable), lambda: issubclass(array.array, Iterable),
 lambda: issubclass(array.array, MutableSequence), lambda: b'42'.decode() == a, lambda: a.tobytes(), lambda: hash(b'1') == 0,
 lambda: bytearray(b'') .join([a]), lambda: b'abc'.replace(array.array('B', b'b'), b'X'), lambda: bytes(a) == b'42', lambda: list(bytearray(a)),
 lambda: bytearray(b'ab') != a, lambda: a != bytearray(b'ab'),
]
for i, t in enumerate(tests):
    try:
        print(i, repr(t()))
    except NotImplementedError as e:  # a refusal would show here
        print(i, "REFUSED", e)
    except Exception as e:
        print(i, type(e).__name__, e)
a = array.array('b', b'abc')
ba = bytearray(b'abc')
if ba == a: print("eq1")
if a == ba: print("eq2")
if not (a != ba): print("ne")
if ba <= a: print("le")
if a < bytearray(b'abd'): print("lt")
print([ba == a, a == ba, a > ba, ba >= a])
print(a in [ba], ba in [a])

# itertools.count(start=, step=) by keyword; CPython's codec aliases for ascii, utf-8 and latin-1; int(bytes, base)
from itertools import count
class T:
    def f(self, step=1):
        c = count(step=step)
        for i in range(3): next(c)
        print(next(c))
    def g(self):
        self.f(step=5)
T().g()
c = count(start=10, step=-2); print(next(c), next(c))
c = count(3, step=2); print(next(c), next(c))
c = count(start=1.5); print(next(c), next(c))
for bad in [lambda: count(1, start=2), lambda: count(foo=1), lambda: count(1, 2, 3)]:
    try: bad()
    except TypeError as e: print(e)
for s in [b'123', b' 12 ', b'123\x00', b'123\xbd', bytearray(b'42'), b'1_000', b'0x1f', '٣'.encode(), b'\xc2\xa012', b'-7', b'', b'z']:
    for base in (None, 10, 16, 0, 36):
        try:
            print(int(s) if base is None else int(s, base))
        except ValueError as e:
            print('VE', e)
for bad in [(b'1', 1), (b'1', 37)]:
    try: int(*bad)
    except ValueError as e: print('VE', e)
try: int(memoryview(b'1'), 10)
except TypeError as e: print('TE', e)
print(int(memoryview(b'12')))
for enc in ['ANSI_X3.4-1968', 'us', 'US-ASCII', 'iso646-us', 'cp367', 'utf8_ucs4', 'U8', 'UTF 8', ' utf-8 ', 'iso_8859_1:1987', 'Latin1', 'csISOLatin1', 'ibm819', 'iso-ir-100', 'ascii!!', 'nope']:
    try:
        print(enc, b'abc'.decode(enc), 'xyz'.encode(enc))
    except LookupError as e:
        print('LE', e)

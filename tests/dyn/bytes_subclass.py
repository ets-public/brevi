import re
class B(bytes):
    pass
class C(bytes):
    def __new__(cls, x, tag='t'):
        o = super().__new__(cls, x)
        o.tag = tag
        return o
    def shout(self):
        return self.upper() + b'!'
b = B(b'abc')
print(type(b).__name__, isinstance(b, bytes), issubclass(B, bytes), b, repr(b), len(b), b[0], b[1:], list(b))
print(b == b'abc', b'abc' == b, hash(b) == hash(b'abc'), b + b'd', type(b + b'd').__name__, b * 2, b.upper(), type(b.upper()).__name__)
print(b'b' in b, 98 in b, b.find(b'c'), bytes(b), type(bytes(b)).__name__, b.decode(), str(b), b.hex())
c = C(b'xy', tag='q'); print(c, c.tag, c.shout(), type(c).__name__)
print(re.match(B(b'a.'), b'abc'), re.match(b'(b)', B(b'bcd')).group(1), re.findall(B(rb'\w'), B(b'a b')))
print(re.compile(B(b'x')).pattern, type(re.compile(B(b'x')).pattern).__name__)
d = {b: 1}; print(d[b'abc'], sorted([B(b'b'), b'a']), b < b'abd', bytearray(b), b.split(b'b'), B(), B(3), B([65, 66]))
print(re.match(rb'a', B(b'ab')).string, type(re.match(rb'a', B(b'ab')).string).__name__)
print(re.sub(b'a', B(b'z'), B(b'aa')), re.split(B(b','), B(b'a,b')))
class B2(bytes):
    pass
class R(bytes):
    def __repr__(self): return 'R!'
    def __eq__(self, o): return True
    __hash__ = bytes.__hash__
    def upper(self): return 'mine'
def t(f):
    try: r = f(); print(type(r).__name__, repr(r))
    except Exception as e: print(type(e).__name__, e)
b = B2(b'ab')
for f in [lambda: memoryview(b).tobytes(), lambda: B2.fromhex('6162'), lambda: bytearray(b), lambda: b''.join([b, B2(b'c')]),
          lambda: b.join([b'x', b'y']), lambda: b + B2(b'c'), lambda: b[::-1], lambda: b * 0, lambda: [c for c in b],
          lambda: {b, b'ab'}, lambda: b in [b'ab'], lambda: b'ab' in {b: 1}, lambda: b < B2(b'b'), lambda: sorted([B2(b'b'), b]),
          lambda: str(b), lambda: f'{b!r} {b}', lambda: bool(B2()), lambda: B2(b'x').__class__ is B2, lambda: type(b) is bytes,
          lambda: R(b'z'), lambda: R(b'z') == 5, lambda: R(b'z').upper(), lambda: R(b'z').lower(), lambda: [R(b'z')],
          lambda: b.__len__(), lambda: bytes.__len__(b), lambda: bytes.upper(b), lambda: b.decode('ascii'), lambda: int(B2(b'12')),
          lambda: B2(b'%d') % 5 if False else 0, lambda: hash(b) == hash(b'ab'), lambda: b.startswith(B2(b'a')),
          lambda: B2('é', 'utf-8'), lambda: B2(range(3)), lambda: isinstance(b, B2), lambda: B2.__mro__, lambda: b.x if False else 0,
          lambda: setattr(b, 'x', 1) or b.x, lambda: vars(b), lambda: b.__dict__, lambda: max(b), lambda: b.count(97),
          lambda: B2.maketrans(b'a', b'b')[97], lambda: b.translate(bytes.maketrans(b'a', b'z')), lambda: b == bytearray(b'ab'),
          lambda: bytearray(b'ab') == b, lambda: memoryview(b) == b, lambda: b.hex(':'), lambda: list(reversed(b))]:
    t(f)
class S(str): pass
B = B2
print(repr(re.sub('y', S('a'), S('xyz'))), repr(re.sub(b'y', B(b'a'), B(b'xyz'))))
print(type(re.sub(b'y', B(b'a'), B(b'xyz'))).__name__)
print(repr(re.sub(b'x', b'\\1', b'x') if False else 0))

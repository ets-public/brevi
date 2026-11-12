import copy
import copy
class BA(bytearray): pass
class N(bytearray):
    def __new__(cls, *a):
        o = super().__new__(cls, *a)
        o.made = True
        return o
class E(bytearray):
    def __eq__(self, o): return True
    def __iadd__(self, o): return 'iadd'
    def __add__(self, o): return 'add'
    def __len__(self): return 42
class H(bytearray):
    def __hash__(self): return 7
def t(f):
    try: r = f(); print(type(r).__name__, repr(r))
    except Exception as e: print(type(e).__name__, e)
x = BA(b'ab'); x.tag = 1
c = copy.copy(x); c.append(99); print(x, c, type(c).__name__, c.tag)
d = copy.deepcopy(x); d.append(100); print(x, d, type(d).__name__, d.tag)
n = N(b'q'); print(n, n.made)
e = E(b'z'); print(e == 1, e + b'a', len(e)); e += b'x'; print(e)
print(hash(H(b'a')), {H(b'a')} and 1)
for f in [lambda: x + 1, lambda: 1 + x, lambda: x * 'a', lambda: 'a' * x, lambda: x - 1, lambda: x < 1, lambda: x.append('a'), lambda: x[5],
          lambda: x.foo, lambda: x.decode('zz'), lambda: len(BA(5)), lambda: BA(-1), lambda: BA('a'), lambda: [1] + x, lambda: 'a' + x, lambda: (1,) + x,
          lambda: x * True, lambda: x * 2**70, lambda: BA(b'a') == BA(b'a'), lambda: BA(b'a') is BA(b'a'), lambda: x.__class__.__name__,
          lambda: isinstance(x, bytes), lambda: issubclass(BA, bytes), lambda: type(x)(b'k'), lambda: BA.__bases__, lambda: BA(b'abc').hex(':'),
          lambda: BA(memoryview(b'mv')), lambda: memoryview(BA(b'xy')).tobytes(), lambda: bytes(memoryview(BA(b'xy'))[::-1]),
          lambda: str(BA(b'a'), 'ascii') if False else 0, lambda: '%s' % BA(b'a'), lambda: '{}'.format(BA(b'a')), lambda: f'{BA(b"a")!s}',
          lambda: print(BA(b'pr')), lambda: [BA(b'l')], lambda: (BA(b't'),), lambda: {'k': BA(b'd')}, lambda: ascii(BA(b'\xff')),
          lambda: repr(BA(b"'")), lambda: BA(b'a') * -1, lambda: int(BA(b'12'), base=10), lambda: int(BA(b'0b11'), 0), lambda: int(BA(b'1_0')),
          lambda: float(BA(b'x')), lambda: int(BA(b'')), lambda: BA(b'ab').__contains__(97), lambda: sum(BA(b'\x01\x02')), lambda: sorted(BA(b'ba')),
          lambda: dict.fromkeys(BA(b'ab')), lambda: BA(b'abc')[::2], lambda: BA(range(3)), lambda: BA(b'a').zfill(3), lambda: BA(b'a,b').split(b',')]:
    t(f)
m = BA(b'abc'); v = memoryview(m); v[0] = 65; print(m)
try: m.append(1)
except BufferError as e: print('BufferError', e)
v.release(); m.append(1); print(m)
class S(str): pass
print(int(S('101'), 2), int(S(' 7 '), base=8))
class I(int):
    def __index__(self): return 3
print(BA(b'a') * I(2), I(2) * BA(b'b'))
class Ix:
    def __index__(self): return 2
print(BA(b'a') * Ix(), Ix() * BA(b'c'))
q = BA(b'r'); q *= 3; print(type(q).__name__, q)

x=BA(b'a'); r=x.__iadd__(b'b'); print(type(r).__name__, r is x, x)
r=x.__imul__(2); print(type(r).__name__, r is x, x)
y=bytearray(b'q'); print(y.__iadd__(b'r') is y, y)
try: y.__iadd__(1)
except TypeError as e: print(e)
b=bytearray(b'a')
c=copy.copy(b); d=copy.deepcopy(b); c.append(1); d.append(2); print(b,c,d, c is b)
m=[b]; m2=copy.deepcopy(m); m2[0].append(3); print(b)
class S(str): pass
print(hash(S('a')) == hash('a'), hash(S('a')) == hash('a'))
d = {S('a'): 1}; print(d['a'], S('a') in d, S('a') in {S('a')})
class T(tuple): pass
print(T((1,)) in {(1,)}, {T((1,)), T((1,))})
x = bytearray(b"'")
print(repr(x), repr(bytearray(b'"\'')))
try: print(x.foo)
except AttributeError as e: print(e)
try: float(bytearray(b'x'))
except ValueError as e: print(e)
try: bytearray(b'a') < 1
except TypeError as e: print(e)
for x in [b'', bytearray(), memoryview(b'')]:
    try: print(x.foo)
    except AttributeError as e: print(e)
    print(hasattr(x, 'bar'), hasattr(x, 'upper'), hasattr(x, 'append'), hasattr(x, 'tobytes'))
class BA(bytearray): pass
class C(bytearray):
    def __init__(self, x, tag='t'):
        super().__init__(x)
        self.tag = tag
    def shout(self): return self.upper() + b'!'
def t(f):
    try: r = f(); print(type(r).__name__, repr(r))
    except Exception as e: print(type(e).__name__, e)
b = BA(b'100')
print(type(b).__name__, isinstance(b, bytearray), issubclass(BA, bytearray), b, repr(b), len(b), b[0], b[1:], list(b))
print(int(b), int(b, 2), float(b))
t(lambda: int(BA(b'A'*16)))
c = C(b'xy', tag='q'); print(c, c.tag, c.shout(), type(c).__name__)
b.append(65); print(b); b.extend(b'zz'); print(b, len(b)); b[0] = 66; print(b); del b[0]; print(b)
b += b'!'; print(type(b).__name__, b)
b *= 2; print(type(b).__name__, b)
m = memoryview(b); print(m.tobytes(), m.obj is b); m.release()
x = BA(b'ab')
for f in [lambda: x == b'ab', lambda: b'ab' == x, lambda: x == bytearray(b'ab'), lambda: hash(x), lambda: x + b'c', lambda: b'c' + x, lambda: x * 2,
          lambda: x.upper(), lambda: x.copy(), lambda: x[::-1], lambda: [c for c in x], lambda: 97 in x, lambda: b'a' in x, lambda: x.find(b'b'),
          lambda: bytes(x), lambda: bytearray(x), lambda: x.decode(), lambda: str(x), lambda: x.hex(), lambda: f'{x!r} {x}', lambda: bool(BA()),
          lambda: BA(), lambda: BA(3), lambda: BA([65,66]), lambda: BA('é','utf-8'), lambda: BA.fromhex('6162'), lambda: type(BA.fromhex('61')),
          lambda: x < b'b', lambda: sorted([BA(b'b'), x]), lambda: x.split(b'a'), lambda: b''.join([x, BA(b'c')]), lambda: x.join([b'1', b'2']),
          lambda: x.pop(), lambda: x, lambda: x.insert(0, 120) or x, lambda: x.remove(120) or x, lambda: x.reverse() or x, lambda: x.clear() or x,
          lambda: setattr(x, 'k', 1) or x.k, lambda: vars(x), lambda: BA.__mro__, lambda: type(x) is bytearray, lambda: x.__class__ is BA,
          lambda: bytearray.__len__(BA(b'abc')), lambda: bytearray.upper(BA(b'a')), lambda: max(BA(b'az')),
          lambda: BA(b'ab').translate(bytes.maketrans(b'a', b'z')), lambda: BA.maketrans(b'a', b'b')[97], lambda: list(reversed(BA(b'ab'))),
          lambda: {BA(b'a')}, lambda: BA(b'abc').__len__(), lambda: BA(b'a b').split(), lambda: int(BA(b' 12 ')), lambda: int(BA(b'ff'), 16),
          lambda: int(BA(b'0x1f'), 0), lambda: float(BA(b'1.5')), lambda: BA(b'xyz')[1], lambda: BA(b'xyz')[-1:],
          lambda: memoryview(BA(b'ab'))[0], lambda: BA(b'ab') != b'ab', lambda: BA(b'a').isalpha(), lambda: BA(b'abc').startswith(BA(b'a')),
          lambda: b'abc'.startswith(BA(b'a')), lambda: b'abc'.replace(BA(b'a'), BA(b'z')), lambda: bytes.fromhex('61') + BA(b'b'), lambda: BA(b'ab') + BA(b'c'),
          lambda: BA(b'a') * 0, lambda: 2 * BA(b'a'), lambda: BA(b'a').__iadd__(b'b'), lambda: BA(b'abc').count(97), lambda: BA(b'abc').index(b'c')]:
    t(f)
y = BA(b'abc'); y[0:2] = b'XYZ'; print(y); y[::2] = b'12'; print(y)
z = BA(b'q'); z2 = z; z += b'r'; print(z2 is z, z2)
import re
print(re.match(b'(a)', BA(b'abc')).group(1), re.findall(rb'\w', BA(b'a b')), re.sub(b'a', b'z', BA(b'aa')))

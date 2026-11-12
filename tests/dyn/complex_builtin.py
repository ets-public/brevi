import math
def t(f):
    try:
        r = f()
        print(repr(r), type(r).__name__)
    except Exception as e:
        print('raises', type(e).__name__, str(e))
for s in ['1+2j', '(1+2j)', ' ( 1-2j ) ', '3j', '-j', '+j', 'j', 'J', '1+j', '1-J', 'inf', 'infj', '-infinityj', 'nan', 'nanj', '1e5j', '1.5e-3+2E2j', '1_000+2_0j',
          '1__0', '_1', '1_', '', ' ', '(', '()', '1+', '1+2', '1+2jj', 'x', '1j+1', '1 + 2j', '\t1j\n', '١٢j', '1e', '1ej', '.5j', '5.j', '(1j', '1j)', '0x1j', '+1.5', '- 1j']:
    t(lambda: complex(s))
class F:
    def __float__(self): return 2.5
class I:
    def __index__(self): return 7
class C:
    def __complex__(self): return 3+4j
class Bad:
    def __complex__(self): return 3
class N: pass
for args in [(), (1,), (1, 2), (1.5, -2), (1j,), (1j, 1j), (1+2j, 3+4j), (2, 1j), (True, False), (F(),), (I(),), (C(),), (Bad(),), (N(),), (None,), (1, None), (1, 'x'), ('1', 2), (10**400,), (1, 10**400), (F(), F()), (I(), I()), (C(), 1)]:
    t(lambda: complex(*args))
t(lambda: complex(real=1, imag=2)); t(lambda: complex(imag=3)); t(lambda: complex(1, real=2)); t(lambda: complex(foo=1)); t(lambda: complex(1, 2, 3))
c = 1+2j
print(type(c), type(c) is complex, isinstance(c, complex), isinstance(c, (int, float)), complex, complex.__name__, issubclass(complex, object))
print(hasattr(c, 'real'), hasattr(c, '__iter__'), hasattr(c, 'conjugate'), hasattr(c, 'foo'), hasattr(c, '__len__'))
t(lambda: c.foo); t(lambda: setattr(c, 'real', 1)); t(lambda: setattr(c, 'x', 1))
print(c.__add__(1), c.__radd__(2.5), c.__add__('x'), c.__eq__(1), c.__lt__(2), c.__complex__() is c, c.__getnewargs__(), c.__abs__(), c.__neg__(), c.__pow__(2), c.__rtruediv__(1))
print(f"{c}", format(c), '%s' % c, '%r' % (c,), str([c, 1j]), {1j: 'a', 1+0j: 'b', 1: 'c'}, {1j, 1j, 2j} == {2j, 1j}, 1j in [1j], (1+0j) in {1: 2}, 2 in {2+0j})
print(sum([1j, 2, 3.5]), sum([1j, 2j], 1j), [x * 1j for x in range(3)], hash(1+0j) == hash(1), hash(2.5+0j) == hash(2.5), hash(-1+0j))
t(lambda: f"{c:.2f}"); t(lambda: '%f' % c); t(lambda: '%d' % c); t(lambda: math.isfinite(c)); t(lambda: len(c)); t(lambda: list(c)); t(lambda: c[0]); t(lambda: c())
import copy
print(copy.copy(c), copy.deepcopy([c]), copy.copy(c) == c)
x = 1j
x += 1
x *= 2
x **= 2
x /= 4j
print(x)
print(1 if 0j else 2, not 1j, 1j and 2, 0j or 3, [1j] * 2, (1j, 2j))
print(complex.conjugate(3-4j), complex(3, 4).imag.is_integer())
import json
t(lambda: json.dumps(1j))

# sys.setswitchinterval/getswitchinterval: Brevi keeps CPython's whole microseconds (set truncates), its
# argument conversion (PyFloat_AsDouble) and its errors; the interval schedules nothing on green threads
import sys, fractions, decimal
print(sys.getswitchinterval())
for v in [1e-6, 1e-7, 2.5e-6, 1, True, fractions.Fraction(1, 3), 0.0049999999, decimal.Decimal('0.1')]:
    sys.setswitchinterval(v); print(repr(sys.getswitchinterval()))
class I:
    def __index__(s): return 3
class F(float): pass
class Bad:
    def __float__(s): raise ValueError('boom')
for v in [0, -1, 'x', None, b'1', 1j, I(), F(0.25), Bad(), 2**2000, [1]]:
    try:
        sys.setswitchinterval(v); print('ok', sys.getswitchinterval())
    except Exception as e:
        print(type(e).__name__, e)
for c in [lambda: sys.setswitchinterval(interval=1), lambda: sys.setswitchinterval(), lambda: sys.setswitchinterval(1, 2), lambda: sys.getswitchinterval(1), lambda: sys.getswitchinterval(x=1)]:
    try: c()
    except Exception as e: print(type(e).__name__, e)
class J(int): pass
sys.setswitchinterval(J(2)); print(sys.getswitchinterval())
class G(float):
    def __float__(s): return 0.5
sys.setswitchinterval(G(0.25)); print(sys.getswitchinterval())
class K(int):
    def __float__(s): return 0.125
sys.setswitchinterval(K(2)); print(sys.getswitchinterval())

import math
def t(f):
    try:
        r = f()
        print(repr(r), type(r).__name__)
    except Exception as e:
        print('raises', type(e).__name__, str(e))

vals = [0j, 1j, -1j, 1+2j, -1.5-2.25j, 3.25-4j, complex(0.0, -0.0), complex(-0.0, 0.0), complex(1e300, 1e300), complex(1e-320, 5),
        complex(float('inf'), 1), complex(1, float('-inf')), complex(float('nan'), 0), complex(2, float('nan')), 1e16j, 100+0j, 1_0j, 1e3j, 07j, .5j]
for v in vals:
    print(repr(v), str(v), v.real, v.imag, bool(v), v.conjugate(), abs(v) if math.isfinite(v.real) and math.isfinite(v.imag) and abs(v.real) < 1e200 else 'big')
    print(hash(v) if v == v else 'nan')
others = [0, -3, 2.5, -0.0, True, 1+1j, 0j, 10**20]
for a in vals[:6]:
    for b in others:
        t(lambda: a + b); t(lambda: b + a); t(lambda: a - b); t(lambda: b - a)
        t(lambda: a * b); t(lambda: b * a); t(lambda: a / b); t(lambda: b / a)
        t(lambda: a ** b); t(lambda: b ** a)
        t(lambda: a == b); t(lambda: b == a); t(lambda: a != b)
for e in [0, 1, 2, 3, -1, -2, 0.5, 2.5, 1j, 1+1j, -0.5j, 101, 100, -100, 1e10]:
    for base in [1j, 1+1j, 0j, 2, -2.5, 0.5-0.5j, 1e200j]:
        t(lambda: base ** e)
t(lambda: (-8) ** 0.5); t(lambda: (-8.0) ** (1/3)); t(lambda: (-2) ** 1.5); t(lambda: pow(-1, 0.5))
t(lambda: 1j < 2j); t(lambda: 1j > 0); t(lambda: sorted([1j, 2])); t(lambda: max(1j, 2j))
t(lambda: 1j // 2); t(lambda: 1j % 2); t(lambda: divmod(1j, 2)); t(lambda: 2 // 1j); t(lambda: round(1j)); t(lambda: ~1j)
t(lambda: int(1j)); t(lambda: float(1j)); t(lambda: math.sqrt(1j)); t(lambda: math.floor(1j))
t(lambda: pow(1j, 2, 3)); t(lambda: pow(2, 1j, 3)); t(lambda: pow(2.0, 3, 1j)); t(lambda: pow(2, 3, 1j)); t(lambda: pow(1j, 2, None))
t(lambda: 1j / 0); t(lambda: 1j / 0j); t(lambda: 0j ** -1); t(lambda: 0j ** 1j); t(lambda: 0j ** 0); t(lambda: 1e200j ** 2); t(lambda: (1e200+1e200j) * (1e200+1e200j))
t(lambda: abs(complex(1e308, 1e308))); t(lambda: abs(complex(float('inf'), float('nan')))); t(lambda: abs(3+4j))
t(lambda: -(1+2j)); t(lambda: +(1+2j)); t(lambda: -0j)

# Numbers the typed build gives exactly as Python does, without leaving 64 bits.
from fractions import Fraction


def rolling(s):
    MOD = (1 << 61) - 1
    h = 0
    for c in s:
        h = (h * 1000003 + ord(c)) % MOD
    return h


def poly(xs, m):
    acc = 0
    for x in xs:
        acc = (acc * 4294967311 + x * x - 7) % m
    return acc


print(rolling("hello world, this is brevi"))
def fmod(a, m):
    return (a * 3 + 1) % m


print(poly([3, 1, 4, 1, 5, 9, 2, 6], 1000000007), poly([10, 20], -97), fmod(5, 2.5), fmod(3037000500, 7.5))
print(int(9.2e18), int(-9.2e18), int(2.9), int(-2.9), int(1e15 + 0.5))
print(round(1234.5678, -2), round(1250.0, -2), round(1350.0, -2), round(-1250.0, -2), round(-0.4, -1))
print(round(5e15 + 0.5, -1), round(123.456, -5), round(9.5e307, -308), round(1.24e308, -307))
print(round(2.675, 2), round(1e-20, 20), round(123.456, 400), round(-0.001, 2), round(0.125, 2), round(0.375, 2))
f = Fraction(12345, 10)
print(round(f, -2), round(f, 1), round(Fraction(25, 1), -1), round(Fraction(35, 1), -1), round(Fraction(-1, 3), 3))
x = 3
print(x << 61, -x << 60, x >> 70, -x >> 70, 7 >> 1, (1 << 62) >> 61)

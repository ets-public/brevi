from fractions import Fraction

def main():
    a = Fraction(1, 3)
    b = Fraction("3/4")
    c = Fraction(5)
    print(a + b, a - b, a * b, a / b, -a, abs(-a))
    print(a < b, a == Fraction(2, 6), c == 5, a > 0)
    print(a.numerator, b.denominator)
    print(float(a), int(b * 4), round(Fraction(7, 2)))
    xs = [Fraction(1, n) for n in range(1, 8)]
    s = sum(xs, Fraction(0))
    print(s, sum(xs))
    print(xs)
    print(Fraction(1, 86400) * 86400, Fraction(1, 127) + Fraction(1, 131))
    print(f"{a:.4f}", str(b), repr(b))
    t = Fraction(0)
    for i in range(1, 30):
        t += Fraction(1, i * (i + 1))
    print(t, t // Fraction(1, 7), t % Fraction(1, 7))
    m = [[Fraction(2), Fraction(1)], [Fraction(1), Fraction(3)]]
    f = m[1][0] / m[0][0]
    m[1] = [m[1][j] - f * m[0][j] for j in range(2)]
    print(m)
    if not Fraction(0):
        print("zero falsy")
    print(Fraction("-6/8"), Fraction("1.25"), max(xs), sorted(xs)[0])

main()

# Any denominator, as CPython's: unit tables, Gaussian elimination.
def rref(m):
    m = [r[:] for r in m]
    for c in range(len(m)):
        p = m[c][c]
        m[c] = [x / p for x in m[c]]
        for i in range(len(m)):
            if i != c:
                f = m[i][c]
                m[i] = [x - f * y for x, y in zip(m[i], m[c])]
    return m


def fmt(x):
    return str(x.numerator) if x.denominator == 1 else "%d/%d" % (x.numerator, x.denominator)


def show(m):
    return "; ".join(" ".join(fmt(x) for x in r) for r in m)


print(show(rref([[Fraction(3), Fraction(7), Fraction(1)], [Fraction(11), Fraction(13), Fraction(17)], [Fraction(19), Fraction(23), Fraction(29)]])))
print(Fraction(1, 86400) + Fraction(1, 127) + Fraction(1, 997))

# Plain code that must build typed and run fast: a seeded generator read with next(), and indexing
# into long texts (ASCII and not).
def rng(seed):
    x = seed
    while True:
        x = (x * 1103515245 + 12345) % 2147483648
        yield x


def walk(seed, n):
    r = rng(seed)
    a = "".join("acgt"[next(r) % 4] for _ in range(n))
    b = "".join("ACGT"[next(r) % 4] for _ in range(n))
    same = 0
    for i in range(n):
        for j in range(0, n, 7):
            if a[i] == b[j].lower():
                same += 1
    return same


print(walk(7, 3000))
r1 = rng(1)
r2 = rng(1)
r1 = rng(2)
print(next(r1), next(r2), next(r1) % 100)

u = "é" * 300 + "abc" + "ü" * 300
print(u[300], u[301], u[-1], u[0], len(u))
s = "x" * 400
t = s + "ö"
print(s[399], t[400], t[-1], s[-1])
s = "y" * 400
print(s[5])

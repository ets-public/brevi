# int() of a local holding text gives a big integer, as in Python: `a, b = parts[0], parts[1]` then
# `str(int(a) + int(b))`, and `n = int(s)` with `s = input()`.
import sys


def main():
    first = input()
    n = int(first)
    print(n + 1)
    print(str(int(first) * 3))
    out = []
    for line in sys.stdin:
        parts = line.split()
        if len(parts) < 2:
            continue
        a, b = parts[0], parts[1]
        out.append(str(int(a) + int(b)))
    print("\n".join(out))
    xs = [2.5, -1e20]
    v = xs[1]
    print(int(v) + 1)
    w = xs[0]
    print(int(w) + 1)


main()

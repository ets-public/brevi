def tower(b, n):
    r = 1
    for _ in range(n):
        r = r * b
    return r
xs = [tower(3, k) for k in range(30, 45)]
print(xs[-1], sum(xs) % 1000000007)
print(1 << 70, int(-1e30))

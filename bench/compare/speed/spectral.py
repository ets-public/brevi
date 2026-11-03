def a(i, j):
    return 1.0 / ((i + j) * (i + j + 1) // 2 + i + 1)

def mul_av(v, n):
    return [sum(a(i, j) * v[j] for j in range(n)) for i in range(n)]

def mul_atv(v, n):
    return [sum(a(j, i) * v[j] for j in range(n)) for i in range(n)]

def mul_atav(v, n):
    return mul_atv(mul_av(v, n), n)

n = int(input())
u = [1.0] * n
v = u
for _ in range(10):
    v = mul_atav(u, n)
    u = mul_atav(v, n)
vbv = 0.0
vv = 0.0
for i in range(n):
    vbv += u[i] * v[i]
    vv += v[i] * v[i]
print(f"{(vbv / vv) ** 0.5:.9f}")

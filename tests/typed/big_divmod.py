def fact(n):
    f = 1
    for i in range(2, n + 1):
        f *= i
    return f
MOD = 1000000007
print(fact(50) % MOD, fact(30) // fact(28), fact(40) // 7, fact(40) % 7)
x = fact(35)
s = 0
while x > 0:
    s += x % 10
    x //= 10
print(s)
h = 0
for c in "abcdefghijklmnopqrstuvwxyz" * 4:
    h = h * 131 + ord(c)
print(h % 1000003, h // 1000000000000000000, -h // 7, -h % 7, h % -11)
a, b = 0, 1
for _ in range(150):
    a, b = b, a + b
print(a % 999999999999, a // b, b // a)

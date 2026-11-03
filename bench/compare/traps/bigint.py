n = int(input())
f = 1
for i in range(2, n + 1):
    f *= i
print(len(str(f)), sum(int(c) for c in str(f)))
print(2 ** 100, 3 ** 50 % 1000000007)
a, b = 0, 1
for _ in range(n):
    a, b = b, a + b
print(a)

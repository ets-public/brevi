import sys
sys.setrecursionlimit(5000)
def fact(n: int) -> int:
    r = 1
    while n > 1:
        r *= n
        n -= 1
    return r
def fib(n):
    if n < 2:
        return n
    return fib(n - 1) + fib(n - 2)
def power_sum(k):
    t = 0
    for i in range(1, 20):
        t = t + i ** 3
    return t
def digits(n):
    return len(str(fact(n)))
def trib(n):
    a, b, c = 0, 0, 1
    for _ in range(n):
        a, b, c = b, c, a + b + c
    return a
print(fact(40), fib(20), power_sum(3), digits(50), trib(80))
print(fact(20) // 3, fact(3) % 4, sum(range(fib(10))))
def fact2(n):
    f = 1
    for i in range(2, n + 1):
        f *= i
    return f
def fib2(n):
    a, b = 0, 1
    for _ in range(n):
        a, b = b, a + b
    return a
def rfact2(n):
    if n <= 1:
        return 1
    return n * rfact2(n - 1)
def count(xs):
    c = 0
    for y in xs:
        c += 1
    return c
print(fact2(30), fib2(100), rfact2(25), count([1, 2]))
print(fact2(5) + 1, fib2(10) * 2, [0] * fib2(3))
x = fact2(25)
print(x > 10 ** 18, x == fact2(25), str(fib2(90)))

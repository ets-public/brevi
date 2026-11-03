def gen(n):
    for i in range(n):
        if i % 2:
            yield i * i
n = int(input())
g = gen(n)
print(next(g), sum(g))
print(list(zip("abc", range(n))), list(enumerate("xy", 1)))
print(any(x > 5 for x in range(n)), all([]), list(map(str, filter(None, [0, 1, 2]))))

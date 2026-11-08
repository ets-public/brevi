def outer():
    def helper(x): return x + 1
    def g(y): return helper(y)
    return g
print(outer()(41))
def outer2():
    def fact(n): return 1 if n < 2 else n * fact(n - 1)
    return fact
print(outer2()(10))
def outer3():
    def fact(n): return 1 if n < 2 else n * fact(n - 1)
    def call(k): return fact(k)
    return call
print(outer3()(5))
def outer4():
    def f(n): return 0 if n == 0 else f(n - 1)
    r = f(5)
    f = len
    def g(x): return f(x)
    return g
print(outer4()([1, 2, 3]))
xs = [(1, 'a'), (2, 'b')]
for a, b in zip([1, 2, 3], 'xy'):
    print(a, b)
else:
    print('done')
print([a * b for a, b in zip([1, 2], [3, 4, 5])])
print({k: v for k, v in zip('ab', [1, 2])})
it = iter([1, 2, 3])
print(list(zip(it, 'a')), list(it))
for a, b in zip(it, it): print(a, b)
def w():
    if (n := 3) > 2:
        return n + 1
print(w())

import math
inf = float('inf')
n = float('nan')
print((n+1) in [n], abs(n) in [n], [n+1] == [n], -n in [n])
x = inf - inf
print(x in [x], [x] == [x], (x,) == (x,))
d = {}
d[inf-inf] = 1
d[inf-inf] = 2
print(len(d))
d2 = {1: n}
print(n in d2.values(), (1, n) in d2.items())
xs = []
xs.append(n)
print(n in xs, xs.index(n), xs.count(n))
lst = [n]*3
print(n in lst, lst.count(n))
def f(): return float('nan')
y = f()
print(y in [y], y in [f()])
print(len({n, n}), len({float('nan'), float('nan')}))
xs.remove(n); print(xs)
print(math.nan in [math.nan], float('nan') in [float('nan')])
s = sum([1.0, n]); print(s in [s], s in [n])

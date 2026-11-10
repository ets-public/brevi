xs = [1, 2]
f = lambda: xs
ys = f()
ys.append(3)
print(xs)
def g():
    a = [[0], [1]]
    h = lambda i: a[i]
    r = h(0)
    r.append(9)
    print(a)
g()

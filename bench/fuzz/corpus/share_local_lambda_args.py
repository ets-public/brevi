def f():
    a = [1]
    g = lambda x: x
    b = g(a)
    b.append(2)
    print(a)
    h = lambda *xs: xs[0] if xs else None
    c = h(a)
    c.append(3)
    print(a)
    k = lambda y=None: y
    d = k(y=a)
    d.append(4)
    print(a)
f()

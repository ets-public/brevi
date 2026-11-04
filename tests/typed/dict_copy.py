d = {k: k*2 for k in range(3)}
c = d.copy()
print(c)
e = {"a": 1}
f = e.copy()
print(f, repr(f))
def g():
    d = {k: k*2 for k in range(3)}
    c = d.copy()
    print(c)
g()
e = {"a": 1}
f = dict(e)
f["b"] = 2
print(f, e)
g = list(e)
print(g)
ks = list(e)
ks.append("z")
print(ks, len(list(e)), sorted(list({"b": 1, "a": 2})))
for k in list(e):
    print(k)
print(list(e) == ["a"])
t = {(1, 2): "a"}
u = t.copy()
u[(3, 4)] = "b"
print(t, u)
xs = [3, 1]
ys = xs.copy()
ys.append(0)
print(xs, ys)

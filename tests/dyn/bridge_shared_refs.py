# A value reached twice in one library call stays one value on both sides, and reversed() of a list is live.
import pickle

a = [1]
x = pickle.loads(pickle.dumps([a, a]))
x[0].append(2)
print(x, x[0] is x[1])

d = {"k": a}
y = pickle.loads(pickle.dumps((d, a)))
y[1].append(3)
print(y)

cyc = [1]
cyc.append(cyc)
z = pickle.loads(pickle.dumps(cyc))
print(z[1] is z, len(z))

orig = [4, 5, 6, 7]
it = iter(orig)
next(it)
it2, b = pickle.loads(pickle.dumps((it, orig)))
b[:] = [10, 11, 12]
print(type(it2).__name__, list(it2))

r = reversed(orig)
next(r)
r2, c = pickle.loads(pickle.dumps((r, orig)))
c[:] = [10, 11, 12]
print(type(r2).__name__, list(r2))

live = [1, 2, 3]
rv = reversed(live)
live[0] = 9
print(list(rv))
shrink = [1, 2, 3, 4]
rs = reversed(shrink)
print(next(rs))
del shrink[1:]
print(list(rs))

dv = {1: "a", 2: "b", 3: "c"}
ri = reversed(dv.values())
next(ri)
print(list(pickle.loads(pickle.dumps(ri))))

# #459: reversed() of a list, a dict and its views reads the live object, as CPython's reverse iterators do
import copy, operator, pickle
a = [1, 2, 3]
r = reversed(a)
a[0] = 9
print(list(r))
d = {1: 'a', 2: 'b'}
rv = reversed(d.values())
d[1] = 'z'
print(list(rv))
rk = reversed(d.items())
d[2] = 'q'
print(list(rk))
for name, it in (("keys", lambda: reversed(d)), ("values", lambda: reversed(d.values())), ("items", lambda: reversed(d.items()))):
    r = it()
    d[3] = 'c'
    for _ in range(2):
        try:
            print(name, next(r))
        except RuntimeError as e:
            print(name, "RuntimeError:", e)
    del d[3]
d = {1: 1}
r = reversed(d)
print(next(r))
d[2] = 2
try:
    next(r)
except RuntimeError as e:
    print("RuntimeError:", e)
d = {1: 1}
r = reversed(d)
print(list(r), list(r))
d[5] = 5
print(list(r))
d = {i: str(i) for i in range(6)}
del d[2]
r = reversed(d.items())
print(next(r), operator.length_hint(r))
print(list(pickle.loads(pickle.dumps(r))), list(copy.copy(r)))
print(list(r), operator.length_hint(r))
class D(dict):
    pass
dd = D(a=1, b=2)
r = reversed(dd)
dd['a'] = 5
print(list(r))
r = reversed(dd.values())
dd['b'] = 7
print(list(r))
print(type(reversed({})).__name__, type(reversed({}.values())).__name__, type(reversed({}.items())).__name__)
r = reversed({})
print(list(r))

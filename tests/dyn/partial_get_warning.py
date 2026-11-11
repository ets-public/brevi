# functools.partial read from an instance warns (CPython 3.13: it will bind in a later version) and does not bind;
# read from the class it does not warn. Shown once per line by the default filters.
import functools, warnings
warnings.showwarning = lambda m, c, f, l, *a: print(c.__name__, l, m)

def f(*a):
    return a

class A:
    m = functools.partial(f, "p")

print(A.m(1))
for i in range(2):
    print(A().m(i))
print(A().m(9))

import re
def f():
    print("called")
    return ""
n = [0]
def g():
    n[0] += 1
    return n[0]
x = f() or "d"
print(x)
y = f() and "z"
print(repr(y))
z = g() or 5
print(z, n)
w = g() and g()
print(w, n)
xs = [3, 0]
print(xs[1] or xs[0], xs[0] or 9)
m = re.match(r"(a)(b)?", "a")
print(m.group(2) or "none", (m.group(1) or "x") + "!")
s = (g() or 1) + (g() or 2)
print(s, n)

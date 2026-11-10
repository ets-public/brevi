def tryit(f, *a):
    try:
        print(repr(f(*a)))
    except ZeroDivisionError as e:
        print("ZDE", e)
    except Exception as e:
        print(type(e).__name__, e)
def p(a, b): return a ** b
try:
    print(0 ** -1)
except ZeroDivisionError as e:
    print("ZDE", e)
x = 0
for b in (-1, -2, -3, 0, 1, 2):
    tryit(p, x, b)
for a, b in ((0.0, -1), (0, -1.5), (0.0, -2.0), (-0.0, -1), (2, -1), (-2, -3), (0, 0), (10, -2), (0, 5)):
    tryit(p, a, b)
tryit(pow, 0, -1)
tryit(pow, 0, -1, 7)
tryit(pow, 3, -1, 7)
tryit(pow, 2, -1, 4)
n = 5
tryit(lambda: (n - 5) ** -1)
vals = [i ** -1 if i else None for i in range(-2, 3)]
print(vals)
y = 0
y **= 2
print(y)
try:
    y **= -1
except ZeroDivisionError as e:
    print("ZDE aug", e)

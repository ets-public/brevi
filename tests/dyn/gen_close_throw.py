def inner():
    try:
        yield "a"
        yield "b"
    finally:
        print("inner finally")

def outer():
    try:
        yield from inner()
        yield "c"
    finally:
        print("outer finally")

o = outer()
print(next(o))
o.close()
print("closed")

o = outer()
print(next(o))
try:
    o.throw(KeyError("k"))
except KeyError as e:
    print("KeyError", e)

def counter():
    n = 0
    try:
        while True:
            got = yield n
            n += 1 if got is None else got
    except ValueError:
        print("value error inside")
        yield -1
    finally:
        print("counter done", n)

c = counter()
print(next(c), c.send(5), next(c))
print(c.throw(ValueError))
print(c.throw(ValueError, "x") if False else "skip")
c.close()

def raiser():
    try:
        yield 1
    finally:
        raise OSError("in finally")
r = raiser(); next(r)
try:
    r.close()
except OSError as e:
    print("OSError", e)

gens = [outer() for _ in range(2)]
for x in gens:
    next(x)
gens = None
print("list dropped")

def fib():
    a, b = 0, 1
    try:
        while True:
            yield a
            a, b = b, a + b
    finally:
        print("fib stop at", a)
print([x for x, _ in zip(fib(), range(5))])
print(sum(x for x in range(5)))
def unstarted():
    try:
        yield 1
    finally:
        print("never printed")
u = unstarted(); u.close(); del u
print("end")

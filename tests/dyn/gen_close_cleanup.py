from contextlib import contextmanager

def g(tag):
    try:
        yield 1
        yield 2
    finally:
        print("finally", tag)

def f():
    x = g("local")
    print(next(x))
    print("leaving f")

f()
print("after f")
for v in g("loop"):
    print("got", v)
    break
print("after loop")
h = g("close")
next(h)
print(h.close())
print(h.close())

def r():
    try:
        yield 1
    except GeneratorExit:
        print("exit seen")
        return 42
    yield 2
k = r(); next(k); print(k.close())

def bad():
    try:
        yield 1
    finally:
        yield 2
b = bad(); next(b)
try:
    b.close()
except RuntimeError as e:
    print("RuntimeError", e)

@contextmanager
def cm(n):
    print("enter", n)
    try:
        yield n * 2
    finally:
        print("exit", n)

with cm(3) as v:
    print("body", v)
try:
    with cm(4):
        raise ValueError("boom")
except ValueError as e:
    print("caught", e)

def w():
    with cm(5) as v:
        yield v
        yield v + 1
it = w()
print(next(it))
del it
print("after del")
glob = g("global")
next(glob)
print("end")

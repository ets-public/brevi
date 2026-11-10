# Positional-only parameters (the `/` marker): keyword calls, missing-argument and
# too-many-argument errors must match CPython on the dynamic build.

def p(a, b, /, c, *, d, **kw):
    return (a, b, c, d, kw)

def try1(*args, **kw):
    try:
        print(p(*args, **kw))
    except TypeError as e:
        print("TE", e)

try1(a=1, b=2, c=3, d=4)
try1(1, 2, 3)
try1(1, 2, 3, d=4)
try1(1, 2, 3, 4, d=5)
try1(1, a=2, b=3, c=4, d=5)
try1(1, a=2, d=4, e=5)

def f(a, b, /, c):
    return (a, b, c)

def tryf(*args, **kw):
    try:
        print(f(*args, **kw))
    except TypeError as e:
        print("TE", e)

tryf(a=1)
tryf(a=1, b=2)
tryf(c=3)
tryf(a=1, d=2)
tryf(1, 2, 3, c=5)
tryf(1, 2, 3, 4, d=5)
tryf(1, a=2, b=3)

def f2(a, b, /, c=3, *, d=4):
    return (a, b, c, d)

def tryf2(*args, **kw):
    try:
        print(f2(*args, **kw))
    except TypeError as e:
        print("TE", e)

tryf2(1, 2, c=8, d=9)
tryf2(1, 2, 3, 4, d=9)

def f3(a, b, /, c=3, *args, d=4):
    return (a, b, c, args, d)

def tryf3(*args, **kw):
    try:
        print(f3(*args, **kw))
    except TypeError as e:
        print("TE", e)

tryf3(1, 2, 3, 4)
tryf3(1, 2, 3, 4, d=9)

def g(a, /, *, b):
    return (a, b)

def tryg(*args, **kw):
    try:
        print(g(*args, **kw))
    except TypeError as e:
        print("TE", e)

tryg(a=1)
tryg(c=1)
tryg(b=2)

def q(a, /, **kw):
    return (a, kw)

def tryq(*args, **kw):
    try:
        print(q(*args, **kw))
    except TypeError as e:
        print("TE", e)

tryq(a=1)
tryq(1, a=2)

def m(a, /, b, *args, c, **kw):
    return (a, b, args, c, kw)

def trym(*args, **kw):
    try:
        print(m(*args, **kw))
    except TypeError as e:
        print("TE", e)

trym(1, a=2, c=3)
trym(1, 2, a=3, c=4)
trym(1, 2, 3, 4, a=5)
trym(1, 2, b=3, a=4)

fn = lambda a, b, /, c=3: (a, b, c)
try:
    print(fn(a=1, b=2))
except TypeError as e:
    print("TE", e)
try:
    print(fn(c=5))
except TypeError as e:
    print("TE", e)

def t(a, /, *, d, **kw):
    return (a, d, kw)

def tryt(*args, **kw):
    try:
        print(t(*args, **kw))
    except TypeError as e:
        print("TE", e)

tryt(1, 2, 3, a=9)
tryt(1, 2, a=9, d=4)
tryt(1, d=4)

def z(*, d):
    return d

try:
    print(z(1))
except TypeError as e:
    print("TE", e)
try:
    print(z(1, d=2))
except TypeError as e:
    print("TE", e)

def w(a=1, *, d):
    return (a, d)

try:
    print(w(1, 2, d=3))
except TypeError as e:
    print("TE", e)

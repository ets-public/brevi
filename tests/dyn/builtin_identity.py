# A C function of CPython's (len, math.sqrt) has no __dict__: assigning or deleting its attributes raises as in
# CPython, but for __module__, which it keeps. A def of Brevi's own copy of a module (random, textwrap, os.path)
# has CPython's __module__, names and __doc__ when CPython's is a plain Python function.
import math
import os.path
import random
import textwrap

for f, n in [(len, "x"), (print, "__doc__"), (len, "__name__"), (math.sqrt, "y"), (abs, "__call__"), (len, "__dict__")]:
    for op in ("set", "del"):
        try:
            if op == "set":
                setattr(f, n, 1)
            else:
                delattr(f, n)
            print(op, n, "ok")
        except (AttributeError, TypeError) as e:
            print(op, n, type(e).__name__, e)
try:
    del len.__class__
except TypeError as e:
    print("TypeError", e)
len.__module__ = "elsewhere"
print(len.__module__)
del len.__module__
print(len.__module__)
math.sqrt.__module__ = "m2"
print(math.sqrt.__module__)

for f in [random.randint, random.choice, random.shuffle, random.seed, textwrap.dedent, textwrap.wrap, os.path.join]:
    print(type(f).__name__, f.__module__, f.__name__, f.__qualname__, (f.__doc__ or "").splitlines()[:1])
print(random.Random._randbelow.__name__)
textwrap.dedent.tag = 3
print(textwrap.dedent.tag)
try:
    random.randint.tag = 1
except AttributeError as e:
    print("AttributeError", e)

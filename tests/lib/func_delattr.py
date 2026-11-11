# del of a def's own attribute, before and after its __dict__ is read (functools.update_wrapper's test does this)
def f():
    pass
f.a = 1
f.b = 2
del f.a
print(hasattr(f, "a"), f.b)
try:
    del f.a
except AttributeError as e:
    print("caught", e)
print(f.__dict__)
del f.b
print(f.__dict__, hasattr(f, "b"))
f.c = 3
del f.c
try:
    f.c
except AttributeError as e:
    print("caught", e)

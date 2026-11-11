import functools
def f(x: int, y: "s" = 1) -> None:
    pass
print(getattr(f, "__annotations__"))
@functools.wraps(f)
def w(*a):
    pass
for n in functools.WRAPPER_ASSIGNMENTS:
    print(n, repr(getattr(w, n, "missing")))

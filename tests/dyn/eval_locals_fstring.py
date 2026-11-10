# eval() of literal text, locals() in a function, f-string fields holding same-quote strings and newlines
# (PEP 701), a nonlocal name the function never uses, SyntaxError and SyntaxWarning as names.
import warnings
print(eval('1, 0 or 1'), eval("  [1, 2][1] + 3"), eval("len('abc')"))
for f in [lambda: eval("0x"), lambda: eval(".. ."), lambda: eval("09"), lambda: eval("1__0"), lambda: eval("x = 1"),
          lambda: eval("1e"), lambda: eval("0 if 1Else 0"), lambda: eval("00"), lambda: eval("0_0"), lambda: eval("1_000.5")]:
    try:
        print("ok", f())
    except SyntaxError as e:
        print("SyntaxError")
with warnings.catch_warnings():
    warnings.simplefilter("ignore", SyntaxWarning)
    print(eval("1 if 1else 0"), eval("1 if 0else 0"))

def outer(x):
    def g():
        x
        return eval("x + 1")
    return g
print(outer(4)())

def k(a, b=2):
    c = a * 2
    def inner():
        return a + c
    if False:
        z = 1
    d = locals()
    del d["inner"]
    return d
print(k(5))

def labels():
    return f"{len(
        'a', 
    ) if False else 'multi-line'} {{x}} {"same quotes"!r} {f"{"nested"}"}"
print(labels())

def nl():
    x = 0
    y = 0
    def f():
        nonlocal x
        nonlocal x, y
        return 1
    return f()
print(nl())

def g():
    yield 1
    return 42
it = g(); next(it)
try: next(it)
except StopIteration as e: print(e.value, e.args)
e = StopIteration(1, 2); print(e.value, e.args)
e.value = 7; print(e.value, e.args)
class S(StopIteration): pass
print(S("x").value, S().value)
try: next(iter(()), )
except StopIteration as e: print(e.value)
def h():
    r = yield from g()
    print("got", r)
    yield 2
print(list(h()))
try: raise StopIteration
except StopIteration as e: print(e.value)
print(hasattr(ValueError(1), "value"))

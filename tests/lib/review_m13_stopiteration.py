try:
    next(iter([]))
except StopIteration as e:
    print(repr(str(e)), e.args, repr(e.value))
for args in [(), (42,), (1, 2), (1, 2, 3)]:
    e = StopIteration(*args)
    print(repr(str(e)), e.args, repr(e.value))
e = StopIteration()
e.value = 99
print(repr(str(e)), e.args, repr(e.value))
def g():
    yield 1
    return 7
it = g()
next(it)
try:
    next(it)
except StopIteration as e:
    print(repr(str(e)), e.args, repr(e.value))

# An endless generator that could raise StopIteration keeps PEP 479 (it is not rewritten into a class).
def nxt(it):
    return next(it)


def gen(s):
    x = s
    while True:
        x = nxt(it) + x
        yield x


def rng(seed):
    x = seed
    while True:
        x = (x * 1103515245 + 12345) % 2147483648
        yield x


it = iter([1, 2])
r = gen(0)
q = rng(3)
print(next(r), next(r), next(q) % 7)
try:
    print(next(r))
except RuntimeError as e:
    print("RuntimeError", e)
except StopIteration:
    print("StopIteration")

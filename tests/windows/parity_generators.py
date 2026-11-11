# Generators on Windows run on fibers. Many live at once must fit (each reserves its stack and commits only
# what it uses), a generator's stack must grow past its first commit, and generators nest.
import sys


def counter(i):
    yield i
    yield i * 2


live = [counter(i) for i in range(5000)]
firsts = [next(g) for g in live]
seconds = [next(g) for g in live]
print(len(live), sum(firsts), sum(seconds))


def depth(n):
    return 0 if n == 0 else 1 + depth(n - 1)


def deep():
    # recursion inside the generator: 900 frames (under CPython's limit of 1000), past the first 64 KB of its stack
    yield depth(900)


print(next(deep()))


def inner(n):
    for k in range(n):
        yield k


def outer():
    for n in range(4):
        yield from inner(n)


print(list(outer()))
print(sum(x for x in range(100000) if x % 7 == 0))
sys.stdout.flush()

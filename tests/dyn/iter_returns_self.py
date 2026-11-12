class It:
    def __init__(self): self.n = 0
    def __iter__(self): return self
    def __next__(self):
        self.n += 1
        if self.n > 3: raise StopIteration
        return self.n
i = It()
print(iter(i) is i, type(iter(i)).__name__, list(iter(It())), [x for x in iter(It())], next(iter(It())))
class Other:
    def __iter__(self): return It()
print(type(iter(Other())).__name__, list(Other()), sorted(iter(Other()), reverse=True))
class Gen:
    def __iter__(self):
        yield 1
        yield 2
g = Gen()
print(list(iter(g)), type(iter(g)).__name__)
class Bad:
    def __iter__(self): return 5
try:
    iter(Bad())
except TypeError as e:
    print(e)
class NoNext:
    def __iter__(self): return NoNext()
try:
    iter(NoNext())
except TypeError as e:
    print(e)

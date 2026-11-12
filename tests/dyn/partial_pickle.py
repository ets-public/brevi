# functools.partial pickles through its own __reduce__ (class, (func,), (func, args, keywords, __dict__)) and
# loads back as the program's own partial, a subclass of it as that subclass; a partial of a lambda cannot
# be pickled, as in CPython.
import functools, pickle

def sig(*a, **k):
    return a, k

class P(functools.partial):
    pass

for cls in (functools.partial, P):
    f = cls(sig, ["x"], bar=[True])
    f.attr = [1]
    for proto in range(pickle.HIGHEST_PROTOCOL + 1):
        g = pickle.loads(pickle.dumps(f, proto))
        print(type(g) is cls, g.func is sig, g.args, g.keywords, g.__dict__, g(2) == f(2))
try:
    pickle.dumps(functools.partial(lambda: 0))
except (pickle.PicklingError, AttributeError) as e:
    print("refused", type(e).__name__)

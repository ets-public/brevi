# copy.copy / copy.deepcopy of an iterator give a new, independent iterator at the same place (shallow: over the
# same sequence; deep: over a deep copy), never the iterator itself; a generator is a TypeError, as in CPython.
import copy, itertools
for name, mk in [("list", lambda: iter([1, [2], 3])), ("tuple", lambda: iter((1, [2], 3))), ("str", lambda: iter("abc")),
                 ("range", lambda: iter(range(4))), ("rev", lambda: reversed([1, 2, 3])), ("gen", lambda: (x for x in [1])),
                 ("map", lambda: map(abs, [1, 2])), ("tee", lambda: itertools.tee([1, 2])[0])]:
    for f in (copy.copy, copy.deepcopy):
        it = mk()
        next(it)
        try:
            c = f(it)
        except Exception as e:
            print(name, f.__name__, type(e).__name__)
            continue
        print(name, f.__name__, c is it, list(c), list(it))

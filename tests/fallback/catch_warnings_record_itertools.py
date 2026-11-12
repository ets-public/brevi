# catch_warnings(record=True) in a program that uses itertools and pickles: the typed build runs it through
# the CPython bridge (the dynamic build runs it natively: tests/dyn/itertools_pickle.py).
import itertools, pickle, warnings

with warnings.catch_warnings(record=True) as ws:
    warnings.simplefilter("always")
    warnings.warn("recorded")
    data = pickle.dumps([1, 2, 3])
print(len(ws), ws[0].category.__name__, list(itertools.islice(pickle.loads(data), 2)))

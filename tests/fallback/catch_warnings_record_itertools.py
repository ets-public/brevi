# catch_warnings(record=True) in a program that uses itertools and pickles or copies is refused by the dynamic
# build itself (its iterators do not pickle yet, and a copy would miss CPython 3.13's DeprecationWarning).
import itertools, pickle, warnings

with warnings.catch_warnings(record=True) as ws:
    warnings.simplefilter("always")
    warnings.warn("recorded")
    data = pickle.dumps([1, 2, 3])
print(len(ws), ws[0].category.__name__, list(itertools.islice(pickle.loads(data), 2)))

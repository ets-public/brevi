# catch_warnings(record=True) in a program that uses itertools and pickles or copies is refused in dynamic
# builds (its iterators do not pickle yet, and a copy would miss CPython 3.13's DeprecationWarning), so
# CPython runs this.
import copy, itertools, warnings

with warnings.catch_warnings(record=True) as ws:
    warnings.simplefilter("always")
    c = copy.copy(itertools.count(2))
print(next(c), len(ws), ws[0].category.__name__)

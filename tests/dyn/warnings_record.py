# warnings.catch_warnings(record=True): the list the block gets is CPython's own, so it sees every warning,
# with the program's file, line and module, and the filters and registries are put back afterwards.
import re, warnings

def where(w):
    return (w.category.__name__, str(w.message), w.filename.rsplit("/", 1)[-1], w.lineno)

def f():
    warnings.warn("from f", RuntimeWarning)

with warnings.catch_warnings(record=True) as ws:
    warnings.simplefilter("always")
    warnings.warn("recorded", UserWarning)
    f()
    re.sub("a", "b", "aaa", 1)
    warnings.warn(DeprecationWarning("an instance"))
    for _ in range(2):
        warnings.warn("twice")
print(len(ws))
for w in ws:
    print(where(w))
with warnings.catch_warnings(record=True) as ws2:
    warnings.simplefilter("default")
    for _ in range(3):
        warnings.warn("once per line")
    warnings.warn("once per line")
print([where(w) for w in ws2])
with warnings.catch_warnings(record=True) as ws3:
    warnings.simplefilter("ignore")
    warnings.warn("ignored")
    warnings.simplefilter("error", UserWarning)
    try:
        warnings.warn("raised")
    except UserWarning as e:
        print("raised", e)
print(ws3, ws is not ws2)

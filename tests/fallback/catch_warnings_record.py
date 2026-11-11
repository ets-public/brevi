# warnings.catch_warnings(record=True) is refused in dynamic builds (its bridged list would be a copy that
# never sees the recorded warnings), so CPython runs this and len(ws) is 1, not 0.
import warnings

with warnings.catch_warnings(record=True) as ws:
    warnings.simplefilter("always")
    warnings.warn("recorded", UserWarning)
print(len(ws), ws[0].category.__name__, str(ws[0].message))

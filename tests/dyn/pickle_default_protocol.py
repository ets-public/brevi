# pickle's default protocol is CPython 3.13's (4), whichever CPython the bridge runs: a newer one (3.14 on)
# defaults to 5. Left out, None, through `from pickle import dumps`, dump and Pickler alike.
import io
import pickle
from pickle import DEFAULT_PROTOCOL, Pickler, dump, dumps

x = [1, "a", (2, 3)]
print(pickle.DEFAULT_PROTOCOL, DEFAULT_PROTOCOL, pickle.HIGHEST_PROTOCOL)
print(pickle.dumps(x))
print(pickle.dumps(x, None))
print(pickle.dumps(x, protocol=None))
print(dumps(x))
print(pickle.dumps(x, 5))
b = io.BytesIO()
pickle.dump(x, b)
dump(x, b, None)
dump(x, b, protocol=2)
print(b.getvalue())
b = io.BytesIO()
Pickler(b).dump(x)
pickle.Pickler(b, None).dump(x)
print(b.getvalue())
for bad in [lambda: pickle.dumps(), lambda: pickle.dumps(x, 9), lambda: pickle.dumps(x, 2, protocol=3)]:
    try:
        bad()
    except (TypeError, ValueError) as e:
        print(type(e).__name__, e)

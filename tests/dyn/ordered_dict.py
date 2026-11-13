# OrderedDict behaviors that gave wrong answers: subclass str/copy, mutation during iteration, ordered ==,
# views, recursive repr, and pickling keeping the type and order.
import pickle
from collections import OrderedDict
from random import randrange


class Sub(OrderedDict):
    pass


class G(OrderedDict):
    def __getitem__(self, k):
        return 99


s = Sub(a=1, b=2)
print(s, str(s), f"{s}", [s], Sub())
c = s.copy()
print(type(c).__name__, c)
print(dict(G(a=1).copy()))


class Key:
    def __hash__(self):
        return randrange(100000)


od = Sub()
for i in range(100):
    od[Key()] = i
try:
    od.copy()
    print("no error")
except KeyError:
    print("KeyError")

od = OrderedDict(a=1, b=2)
try:
    for k in od:
        od["z"] = 0
except RuntimeError as e:
    print("RuntimeError:", e)
print(OrderedDict(a=1, b=2) == OrderedDict(b=2, a=1), OrderedDict(a=1, b=2) == dict(b=2, a=1))
print(od.keys(), od.values(), od.items())
r = OrderedDict()
r[1] = r
print(r)

od = OrderedDict(a=1, b=2, c=3)
od.move_to_end("a")
od[1] = od
for proto in range(pickle.HIGHEST_PROTOCOL + 1):
    dup = pickle.loads(pickle.dumps(od, proto))
    print(proto, type(dup).__name__, list(dup), dup[1] is dup, dup is od)
print(pickle.loads(pickle.dumps([OrderedDict(z=1), {"q": OrderedDict()}])))

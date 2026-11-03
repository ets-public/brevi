# `return None` on one path and a list on another: an empty list is not None.
def parse(ts):
    r = []
    for t in ts:
        if not t.isdigit():
            return None
        r.append(int(t))
    return r


for ts in [[], ["1", "2"], ["x"]]:
    x = parse(ts)
    print(x is None, x)
    if x is not None:
        print(len(x), sum(x))


def lookup(d, k):
    if k not in d:
        return None
    return d[k]


m = {"a": {}, "b": {"x": 1}}
for k in ["a", "b", "c"]:
    v = lookup(m, k)
    print(k, v is None, v)

import sys
from collections import OrderedDict

lines = sys.stdin.read().splitlines()
cap = int(lines[0].strip())
cache = OrderedDict()
out = []
for line in lines[1:]:
    p = line.split(None, 2)
    if not p:
        continue
    if p[0] == "put" and len(p) >= 3:
        k, v = p[1], p[2]
        if k in cache:
            cache.move_to_end(k)
            cache[k] = v
        else:
            if len(cache) >= cap:
                cache.popitem(last=False)
            cache[k] = v
    elif p[0] == "get" and len(p) >= 2:
        k = p[1]
        if k in cache:
            cache.move_to_end(k)
            out.append(cache[k])
        else:
            out.append("-1")
if out:
    print("\n".join(out))

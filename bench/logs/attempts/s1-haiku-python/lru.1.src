import sys
from collections import OrderedDict

lines = sys.stdin.read().splitlines()
cap = int(lines[0].strip())
cache = OrderedDict()
out = []
for line in lines[1:]:
    p = line.split()
    if not p:
        continue
    if p[0] == "put":
        key, val = p[1], p[2]
        if key in cache:
            cache.move_to_end(key)
        elif len(cache) >= cap:
            cache.popitem(last=False)
        cache[key] = val
    elif p[0] == "get":
        key = p[1]
        if key in cache:
            cache.move_to_end(key)
            out.append(cache[key])
        else:
            out.append("-1")
for o in out:
    print(o)

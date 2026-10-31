import sys
from collections import OrderedDict

lines = [l.strip() for l in sys.stdin.read().splitlines() if l.strip()]
cap = int(lines[0])
cache = OrderedDict()

for line in lines[1:]:
    p = line.split()
    if p[0] == "put":
        key, val = p[1], p[2]
        if key in cache:
            cache[key] = val
            cache.move_to_end(key)
        else:
            if len(cache) >= cap:
                cache.popitem(last=False)
            cache[key] = val
    elif p[0] == "get":
        key = p[1]
        if key in cache:
            cache.move_to_end(key)
            print(cache[key])
        else:
            print(-1)

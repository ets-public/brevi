import sys
from collections import OrderedDict

lines = [l.split() for l in sys.stdin if l.strip()]
cap = int(lines[0][0])
cache = OrderedDict()
for parts in lines[1:]:
    if parts[0] == "put":
        key, val = parts[1], parts[2]
        if key in cache:
            cache.move_to_end(key)
        elif len(cache) >= cap:
            cache.popitem(last=False)
        cache[key] = val
    elif parts[0] == "get":
        key = parts[1]
        if key in cache:
            cache.move_to_end(key)
            print(cache[key])
        else:
            print(-1)

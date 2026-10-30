import sys
from collections import OrderedDict

lines = [l for l in sys.stdin.read().split('\n') if l.strip()]
cap = int(lines[0].strip())
cache = OrderedDict()
out = []
for line in lines[1:]:
    parts = line.split()
    if parts[0] == 'put':
        key, val = parts[1], parts[2]
        if key in cache:
            cache[key] = val
            cache.move_to_end(key)
        else:
            if len(cache) >= cap:
                cache.popitem(last=False)
            cache[key] = val
    elif parts[0] == 'get':
        key = parts[1]
        if key in cache:
            cache.move_to_end(key)
            out.append(cache[key])
        else:
            out.append('-1')
for o in out:
    print(o)

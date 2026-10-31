import sys
from collections import OrderedDict
lines = sys.stdin.read().split('\n')
cap = int(lines[0])
c = OrderedDict()
for line in lines[1:]:
    p = line.split()
    if not p:
        continue
    if p[0] == 'put':
        k, v = p[1], p[2]
        if k in c:
            c[k] = v
            c.move_to_end(k)
        else:
            if len(c) >= cap:
                c.popitem(last=False)
            c[k] = v
    elif p[0] == 'get':
        k = p[1]
        if k in c:
            c.move_to_end(k)
            print(c[k])
        else:
            print(-1)

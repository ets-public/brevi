import sys
from collections import OrderedDict
L=sys.stdin.read().splitlines()
n=int(L[0]);c=OrderedDict()
for l in L[1:]:
    p=l.split()
    if not p: continue
    if p[0]=='put':
        if p[1] in c: c.move_to_end(p[1])
        elif len(c)>=n: c.popitem(last=False)
        c[p[1]]=p[2]
    else:
        if p[1] in c: c.move_to_end(p[1]);print(c[p[1]])
        else: print(-1)

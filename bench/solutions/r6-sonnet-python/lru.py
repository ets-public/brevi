import sys
from collections import OrderedDict
L=sys.stdin.read().split('\n')
n=int(L[0]);c=OrderedDict()
for l in L[1:]:
    p=l.split()
    if not p: continue
    if p[0]=='put':
        k=p[1]
        if k in c: c.move_to_end(k)
        elif len(c)>=n: c.popitem(last=False)
        c[k]=p[2] if len(p)>2 else ''
    else:
        k=p[1]
        if k in c: c.move_to_end(k);print(c[k])
        else: print(-1)

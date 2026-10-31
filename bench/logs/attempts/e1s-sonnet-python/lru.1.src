import sys
from collections import OrderedDict
L=sys.stdin.read().split('\n')
n=int(L[0]);c=OrderedDict()
for l in L[1:]:
    p=l.split()
    if not p: continue
    if p[0]=='put':
        k,v=p[1],p[2]
        if k in c: c.move_to_end(k);c[k]=v
        else:
            if len(c)>=n: c.popitem(last=False)
            c[k]=v
    elif p[0]=='get':
        k=p[1]
        if k in c: c.move_to_end(k);print(c[k])
        else: print(-1)

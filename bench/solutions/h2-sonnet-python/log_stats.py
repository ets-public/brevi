import sys,math
from collections import defaultdict
d=defaultdict(list)
for l in sys.stdin:
    p=l.split()
    if len(p)<4: continue
    d[p[1]].append((int(p[3]),int(p[2])))
for k in sorted(d):
    v=d[k]; n=len(v); ms=sorted(x[0] for x in v)
    idx=-(-n*9//10)
    err=sum(1 for x in v if x[1]>=500)*100/n
    print(f"{k} {n} {sum(ms)/n:.1f} {ms[idx-1]} {err:.1f}%")

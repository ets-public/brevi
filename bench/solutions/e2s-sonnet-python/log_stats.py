import sys
from decimal import Decimal,ROUND_HALF_UP
d={}
for l in sys.stdin:
    p=l.split()
    if len(p)<4: continue
    d.setdefault(p[1],[]).append((int(p[2]),int(p[3])))
def r1(x): return str(Decimal(x).quantize(Decimal("0.1"),rounding=ROUND_HALF_UP))
for k in sorted(d):
    v=d[k];n=len(v);ms=sorted(x[1] for x in v)
    avg=Decimal(sum(ms))/n
    p90=ms[(9*n+9)//10-1]
    e=Decimal(100*sum(1 for x in v if x[0]>=500))/n
    print(k,n,r1(avg),p90,r1(e)+"%")

import sys
from decimal import Decimal
from collections import defaultdict
inn=defaultdict(Decimal);out=defaultdict(Decimal);bal=defaultdict(Decimal)
for l in sys.stdin.read().split('\n')[1:]:
    l=l.strip()
    if not l: continue
    d,a,m=l.split(',')
    m=Decimal(m.strip());mo=d.strip()[:7]
    inn[mo]+=0;out[mo]+=0
    if m>0: inn[mo]+=m
    else: out[mo]+=-m
    bal[a.strip()]+=m
for mo in sorted(inn):
    print(f"{mo} {inn[mo]:.2f} {out[mo]:.2f} {inn[mo]-out[mo]:.2f}")
if bal:
    a=min(bal,key=lambda k:(-bal[k],k))
    print(f"top {a} {bal[a]:.2f}")

import sys
from decimal import Decimal
from collections import defaultdict
lines=[l.strip() for l in sys.stdin if l.strip()]
inn=defaultdict(Decimal);out=defaultdict(Decimal);bal=defaultdict(Decimal)
for l in lines[1:]:
    d,a,m=[x.strip() for x in l.split(',')]
    m=Decimal(m);k=d[:7]
    inn[k]+=0;out[k]+=0
    if m>0:inn[k]+=m
    else:out[k]+=-m
    bal[a]+=m
for k in sorted(inn):
    print(f"{k} {inn[k]:.2f} {out[k]:.2f} {inn[k]-out[k]:.2f}")
if bal:
    a=min(bal,key=lambda x:(-bal[x],x))
    print(f"top {a} {bal[a]:.2f}")

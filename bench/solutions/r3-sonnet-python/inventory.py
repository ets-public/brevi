import sys,csv
from decimal import Decimal
r=csv.reader(sys.stdin);next(r)
d={}
for row in r:
    if len(row)<3: continue
    d[row[0]]=d.get(row[0],Decimal(0))+int(row[1].strip())*Decimal(row[2].strip())
for k,v in sorted(d.items(),key=lambda x:(-x[1],x[0])):
    print(f"{k}: {v:.2f}")
